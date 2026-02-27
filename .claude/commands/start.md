1. First, check the current date with `date '+%a %Y/%m/%d %I:%M:%S %p %Z week %V'`.
2. Ensure today's journal file exists at `journal/YYYY/MM/DD.md` (based on today's date).
   - If missing, create it from `template/daily.md` (Mon-Fri) or `template/weekend.md` (Sat/Sun).
3. Start (or confirm) the embedding service:
   - Run `./start-server.sh` in the background.
4. Update the search index:
   - Run `sleep 10 && ./index-journal.sh` (incremental).
   - If indices are corrupted or missing, prefer `./reindex-rag.sh` (full rebuild).
5. Monday check (only if today is Monday):
   - Compute the previous ISO week (YYYY-WW) and check for `journal/topics/weekly_retro/YYYY-WW.md`.
   - If it does not exist, run the `weekly-retro-analyzer` agent for the previous week (Mon-Sun) to create it.
   - After the retro exists, reset the rolling daily summary for the new week by recreating `journal/topics/daily_summary.md` from `template/daily_summary.md`.
   - Generate weekly artifacts (optional):
     - `./generate-weekly-weight-graph.sh`
     - `./query-frontmatter.sh --lint --last-days 30 --format table`
6. Then follow your normal rules and read any required context files.

