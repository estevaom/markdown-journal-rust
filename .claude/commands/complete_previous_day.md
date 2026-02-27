This command is used when the user missed capturing the end of the previous day.

1. Check the current date with `date '+%a %Y/%m/%d %I:%M:%S %p %Z week %V'`.
2. Compute yesterday's date (YYYY-MM-DD) and locate `journal/YYYY/MM/DD.md` for that day.
3. If the file does not exist, create it from `template/daily.md` or `template/weekend.md` (based on the weekday).
4. Ask the user for any missing details you should log for that day, then update the file (do not invent).
5. Run the `journal-field-completer` agent on yesterday's file.
6. Run the `daily-summary` agent on yesterday's file.
7. Run the `commit` command to commit and push changes.

