# -*- coding: utf-8 -*-
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


def repo_root() -> Path:
    # This file lives at: <repo>/.tech/code/scripts/weight-analysis/weight_analysis.py
    return Path(__file__).resolve().parents[4]


def fetch_weight_data(root: Path) -> list[dict]:
    cmd = [
        str(root / "query-frontmatter.sh"),
        "--fields",
        "weight_kg",
        "--format",
        "json",
    ]

    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        cwd=str(root),
    )

    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "frontmatter query failed")

    raw_data = json.loads(result.stdout)

    weight_data: list[dict] = []
    for entry in raw_data:
        weight = entry.get("weight_kg")
        if weight is None:
            continue
        try:
            weight = float(weight)
        except (TypeError, ValueError):
            continue
        weight_data.append({"date": entry["date"], "weight_kg": weight})

    return weight_data


def parse_milestones(value: str | None) -> list[float]:
    if not value:
        return []
    out: list[float] = []
    for part in value.split(","):
        part = part.strip()
        if not part:
            continue
        out.append(float(part))
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate a weight progress graph from journal frontmatter.")
    parser.add_argument(
        "--output",
        default="weight_progress.png",
        help="Output PNG path (relative to repo root unless absolute). Default: weight_progress.png",
    )
    parser.add_argument(
        "--show",
        action="store_true",
        help="Display the graph interactively (default: off).",
    )
    parser.add_argument(
        "--target",
        type=float,
        default=None,
        help="Optional target weight (kg) to draw as a horizontal line.",
    )
    parser.add_argument(
        "--milestones",
        type=str,
        default=None,
        help="Optional comma-separated milestone weights (kg) to draw as horizontal lines.",
    )
    args = parser.parse_args()

    root = repo_root()
    output_path = Path(args.output)
    if not output_path.is_absolute():
        output_path = root / output_path
    output_path.parent.mkdir(parents=True, exist_ok=True)

    try:
        import matplotlib

        if not args.show:
            matplotlib.use("Agg")

        import matplotlib.dates as mdates
        import matplotlib.pyplot as plt
        import numpy as np
        import pandas as pd
    except Exception as e:
        print(f"Missing Python dependencies for weight graph: {e}", file=sys.stderr)
        print("Setup: cd .tech/code/scripts/weight-analysis && ./setup_venv.sh", file=sys.stderr)
        return 1

    try:
        weight_data = fetch_weight_data(root)
    except Exception as e:
        print(f"Error fetching weight data: {e}", file=sys.stderr)
        return 1

    if not weight_data:
        print("No weight_kg entries found in journal frontmatter.", file=sys.stderr)
        return 1

    df = pd.DataFrame(weight_data)
    df["date"] = pd.to_datetime(df["date"])
    df = df.sort_values("date").reset_index(drop=True)

    plt.figure(figsize=(14, 8))
    plt.style.use("default")

    plt.plot(
        df["date"],
        df["weight_kg"],
        "b-",
        linewidth=1.5,
        marker="o",
        markersize=3,
        alpha=0.6,
        label="Daily Weight",
    )

    df["weight_7day_avg"] = df["weight_kg"].rolling(window=7, center=True).mean()
    plt.plot(
        df["date"],
        df["weight_7day_avg"],
        color="purple",
        linewidth=2.5,
        alpha=0.8,
        label="7-Day Average",
    )

    if len(df) >= 2:
        z = np.polyfit(range(len(df)), df["weight_kg"], 1)
        p = np.poly1d(z)
        trend_y = p(range(len(df)))
        trend_label = "Trend (slope: {:.3f} kg/day)".format(z[0])
        plt.plot(df["date"], trend_y, "r--", linewidth=2, alpha=0.8, label=trend_label)

    if args.target is not None:
        plt.axhline(
            y=args.target,
            color="green",
            linestyle=":",
            linewidth=2,
            alpha=0.7,
            label=f"Target: {args.target:g} kg",
        )

    for m in parse_milestones(args.milestones):
        plt.axhline(
            y=m,
            color="orange",
            linestyle="-.",
            linewidth=1,
            alpha=0.5,
            label=f"Milestone: {m:g} kg",
        )

    peak_idx = int(df["weight_kg"].idxmax())
    current_idx = int(df.index[-1])

    peak_label = f"Peak: {df.loc[peak_idx, 'weight_kg']:.1f} kg"
    current_label = f"Current: {df.loc[current_idx, 'weight_kg']:.1f} kg"

    plt.scatter(
        df.loc[peak_idx, "date"],
        df.loc[peak_idx, "weight_kg"],
        color="red",
        s=100,
        zorder=5,
        label=peak_label,
    )
    plt.scatter(
        df.loc[current_idx, "date"],
        df.loc[current_idx, "weight_kg"],
        color="darkgreen",
        s=100,
        zorder=5,
        label=current_label,
    )

    total_days = (df["date"].iloc[-1] - df["date"].iloc[0]).days
    from_peak = df.loc[current_idx, "weight_kg"] - df.loc[peak_idx, "weight_kg"]

    recent_df = df.tail(14)
    recent_rate = 0.0
    if len(recent_df) > 1:
        recent_delta = recent_df["weight_kg"].iloc[-1] - recent_df["weight_kg"].iloc[0]
        recent_days = (recent_df["date"].iloc[-1] - recent_df["date"].iloc[0]).days
        recent_rate = (recent_delta / recent_days) if recent_days > 0 else 0.0

    date_range = f"({df['date'].iloc[0].strftime('%B %d')} - {df['date'].iloc[-1].strftime('%B %d, %Y')})"
    plt.title(f"Weight Progress Over Time\n{date_range}", fontsize=16, fontweight="bold", pad=20)
    plt.xlabel("Date", fontsize=12)
    plt.ylabel("Weight (kg)", fontsize=12)
    plt.grid(True, alpha=0.3)
    plt.legend(loc="upper right", fontsize=10)

    plt.xticks(rotation=45)
    plt.gca().xaxis.set_major_locator(mdates.WeekdayLocator(interval=1))
    plt.gca().xaxis.set_major_formatter(mdates.DateFormatter("%m-%d"))

    stats_text = (
        "Summary:\n"
        f"• Peak: {df.loc[peak_idx, 'weight_kg']:.1f} kg ({df.loc[peak_idx, 'date'].strftime('%Y-%m-%d')})\n"
        f"• Current: {df.loc[current_idx, 'weight_kg']:.1f} kg ({df.loc[current_idx, 'date'].strftime('%Y-%m-%d')})\n"
        f"• Delta (current - peak): {from_peak:+.1f} kg\n"
        f"• Span: {total_days} days\n"
        f"• Recent rate (14d): {recent_rate:+.3f} kg/day"
    )

    if args.target is not None and recent_rate != 0:
        remaining = args.target - df.loc[current_idx, "weight_kg"]
        est_days = remaining / recent_rate
        stats_text += f"\n• To target (recent rate): {est_days:+.0f} days"

    plt.text(
        0.02,
        0.02,
        stats_text,
        transform=plt.gca().transAxes,
        fontsize=10,
        verticalalignment="bottom",
        bbox=dict(boxstyle="round", facecolor="wheat", alpha=0.8),
    )

    plt.tight_layout()
    plt.savefig(output_path, dpi=300, bbox_inches="tight")

    if args.show:
        plt.show()

    print(f"Graph saved: {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
