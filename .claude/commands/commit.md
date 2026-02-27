1. Check the current date with `date '+%a %Y/%m/%d %I:%M:%S %p %Z week %V'`.
2. Confirm today's journal file path: `journal/YYYY/MM/DD.md`.
3. If it's after 7pm local time:
   - Run the `journal-field-completer` agent on today's file (fill missing `tags`, `triggers`, and reflection fields only from facts in the entry).
   - Run the `daily-summary` agent for today's file (append exactly 10 bullets to `journal/topics/daily_summary.md`).
4. If it's before 7pm local time:
   - Do not run automated end-of-day completion unless the user explicitly requests it.
5. Commit and push:
   - `git status`
   - `git add -A`
   - `git commit -m "journal: YYYY-MM-DD"`
   - `git push`

