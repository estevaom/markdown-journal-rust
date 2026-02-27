use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Identifies and filters out template boilerplate from journal entries
pub struct TemplateFilter {
    template_lines: HashSet<String>,
}

impl TemplateFilter {
    pub fn new() -> Self {
        let mut template_lines = HashSet::new();

        // Load templates from disk at runtime so the filter stays in sync without recompiling.
        if let Some(root) = Self::repo_root() {
            for rel in ["template/daily.md", "template/weekend.md"] {
                Self::load_template_lines(&root.join(rel), &mut template_lines);
            }
        }

        // Backwards-compat: keep removing known template boilerplate even if the template evolves.
        for line in [
            "## I. Work Responsibilities & Goals (Mon-Fri)",
            "## II. Primary Focus Activities: [Declared Primary Focus from above]",
            "## III. Nice-to-Haves / Other Minor Tasks",
            "## IV. Progress Toward Broader Goals",
            "## V. End-of-Day Reflection",
            "### A. If AI Study:",
            "### B. If Rust Study:",
            "### C. If Other Focused Activity (e.g., NixOS Rice, Specific Project):",
            "### C. If Other Focused Activity:",
            "- Main Work Goal(s) for Today:",
            "- Key Work Tasks:",
            "- Learning Objective(s) (Review `journal/topics/ai_study_backlog.md` with Cline if needed):",
            "- Project Task(s) (if any):",
            "- Key Questions for AI / Discussion Points:",
            "- Time Allotted:",
            "- Goal for this session:",
            "- Specific Learning Focus:",
            "- Reflection/Notes:",
            "- What went well today (Work, Primary Focus, Personal)?",
            "- Challenges faced & how they were handled?",
            "- Key learnings (Technical, Rust, Personal, etc.)?",
            "- How did the overall balance feel today (Work/Focus/Relaxation/Other Activities)?",
            "- Adjustments or intentions for tomorrow?",
            "- Gratitude Moment:",
        ] {
            template_lines.insert(line.to_string());
        }

        Self { template_lines }
    }

    fn repo_root() -> Option<PathBuf> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.ancestors().nth(5).map(|p| p.to_path_buf())
    }

    fn load_template_lines(path: &Path, out: &mut HashSet<String>) {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let stripped = Self::strip_frontmatter(&content);
        for line in stripped.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            out.insert(trimmed.to_string());
        }
    }

    fn strip_frontmatter(content: &str) -> &str {
        // Only strip if the file starts with a YAML frontmatter fence.
        let mut lines = content.lines();
        if lines.next().map(|l| l.trim()) != Some("---") {
            return content;
        }

        let mut idx = 0usize;
        let bytes = content.as_bytes();

        // Find the end of the opening fence line.
        while idx < bytes.len() && bytes[idx] != b'\n' {
            idx += 1;
        }
        if idx < bytes.len() {
            idx += 1;
        }

        // Find the closing fence line.
        while idx < bytes.len() {
            if bytes[idx] == b'\n' {
                let start = idx + 1;
                if start + 3 <= bytes.len() && &bytes[start..start + 3] == b"---" {
                    let mut end = start + 3;
                    if end < bytes.len() && bytes[end] == b'\r' {
                        end += 1;
                    }
                    if end < bytes.len() && bytes[end] == b'\n' {
                        end += 1;
                    }
                    return &content[end..];
                }
            }
            idx += 1;
        }

        // If we can't find the closing fence, don't strip anything.
        content
    }

    fn is_placeholder_line(trimmed: &str) -> bool {
        if trimmed.is_empty() {
            return true;
        }

        if trimmed == "-" || trimmed == "- [ ]" {
            return true;
        }

        // Unfilled checkboxes in templates.
        if trimmed.starts_with("- [ ]") && trimmed.len() == "- [ ]".len() {
            return true;
        }

        // Common placeholders used in templates.
        if trimmed.contains("___")
            || trimmed.contains("YYYY-MM-DD")
            || trimmed.contains("[Weekday/Weekend]")
            || trimmed.contains("[User confirms")
            || trimmed.contains("[Work/Relaxation/Other]")
        {
            return true;
        }

        false
    }

    fn is_boilerplate_line(&self, line: &str) -> bool {
        let trimmed = line.trim();
        Self::is_placeholder_line(trimmed) || self.template_lines.contains(trimmed)
    }

    /// Process content and return cleaned version with template noise removed
    pub fn clean_content(&self, content: &str) -> String {
        let content = Self::strip_frontmatter(content);

        let mut cleaned = String::new();
        let mut section_header: Option<String> = None;
        let mut section_body = String::new();
        let mut has_meaningful = false;

        for raw_line in content.lines() {
            let line = raw_line.trim_end();
            let is_header = line.trim_start().starts_with('#');

            if is_header {
                // Flush previous section.
                if has_meaningful {
                    if let Some(header) = section_header.take() {
                        cleaned.push_str(&header);
                        cleaned.push('\n');
                    }
                    cleaned.push_str(section_body.trim_end());
                    cleaned.push('\n');
                    cleaned.push('\n');
                }

                section_header = Some(line.trim().to_string());
                section_body.clear();
                has_meaningful = false;
                continue;
            }

            if self.is_boilerplate_line(line) {
                continue;
            }

            section_body.push_str(line);
            section_body.push('\n');

            if !line.trim().is_empty() {
                has_meaningful = true;
            }
        }

        // Flush last section.
        if has_meaningful {
            if let Some(header) = section_header.take() {
                cleaned.push_str(&header);
                cleaned.push('\n');
            }
            cleaned.push_str(section_body.trim_end());
        }

        self.remove_excessive_whitespace(&cleaned)
    }

    /// Remove excessive whitespace while preserving paragraph structure
    fn remove_excessive_whitespace(&self, content: &str) -> String {
        let mut result = String::new();
        let mut consecutive_empty = 0;

        for line in content.lines() {
            if line.trim().is_empty() {
                consecutive_empty += 1;
                if consecutive_empty <= 2 {
                    result.push('\n');
                }
            } else {
                consecutive_empty = 0;
                result.push_str(line);
                result.push('\n');
            }
        }

        result.trim().to_string()
    }

    /// Extract chunks by meaningful sections, skipping template noise
    pub fn extract_chunks(&self, content: &str, max_chunk_size: usize) -> Vec<String> {
        let cleaned = self.clean_content(content);
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();

        for line in cleaned.lines() {
            // Start new chunk on headers
            if line.starts_with('#') && !current_chunk.is_empty() {
                if current_chunk.len() > 100 {
                    // Minimum chunk size
                    chunks.push(current_chunk.trim().to_string());
                }
                current_chunk.clear();
            }

            current_chunk.push_str(line);
            current_chunk.push('\n');

            // Split if chunk gets too large
            if current_chunk.len() > max_chunk_size {
                chunks.push(current_chunk.trim().to_string());
                current_chunk.clear();
            }
        }

        // Don't forget the last chunk
        if current_chunk.len() > 100 {
            chunks.push(current_chunk.trim().to_string());
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_removes_unfilled_template_sections() {
        let filter = TemplateFilter::new();
        let content = r#"
# Daily Reflection - YYYY-MM-DD

## Morning Check-in
Actually wrote something real here.

## Template Section
- Placeholder: ___
- [ ]

## Real Content
This section has actual content worth indexing.
"#;
        
        let cleaned = filter.clean_content(content);
        assert!(!cleaned.contains("Placeholder: ___"));
        assert!(cleaned.contains("Real Content"));
        assert!(cleaned.contains("Actually wrote something real"));
    }
}
