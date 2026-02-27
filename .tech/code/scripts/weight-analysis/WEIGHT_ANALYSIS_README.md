# Weight Analysis Tool

Generates a simple weight progress chart using `weight_kg` values from your journal frontmatter.

## Setup

```bash
cd .tech/code/scripts/weight-analysis
./setup_venv.sh
```

## Run

```bash
cd .tech/code/scripts/weight-analysis
source .venv/bin/activate
python weight_analysis.py --output ../../../../weight_progress.png
deactivate
```

## Notes

- Data is pulled via `./query-frontmatter.sh --fields weight_kg --format json`.
- You can optionally add a target line with `--target` and extra milestone lines with `--milestones`.
