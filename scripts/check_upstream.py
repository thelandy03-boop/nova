#!/usr/bin/env python3
"""
Monitor de actualizaciones para repositorios upstream de Nova:
- storytold/soundcraft (Motor Pro Tools & VST3)
- fabox-os/Pro-Wave (UI Logic Pro)
"""

import json
import os
import subprocess
import sys

STATE_FILE = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), ".upstream-state.json")

def get_latest_commit(repo):
    cmd = ["gh", "api", f"repos/{repo}/commits/main", "--jq", "{sha: .sha, message: .commit.message, author: .commit.author.name, date: .commit.author.date}"]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, check=True)
        return json.loads(res.stdout)
    except Exception as e:
        return None

def main():
    if not os.path.exists(STATE_FILE):
        state = {"soundcraft": "", "prowave": ""}
    else:
        with open(STATE_FILE, "r") as f:
            state = json.load(f)

    repos = {
        "soundcraft": "storytold/soundcraft",
        "prowave": "fabox-os/Pro-Wave"
    }

    report = []
    has_updates = False

    for key, repo_name in repos.items():
        latest = get_latest_commit(repo_name)
        if not latest:
            continue

        last_sha = state.get(key, "")
        if latest["sha"] != last_sha:
            has_updates = True
            report.append({
                "repo": repo_name,
                "key": key,
                "old_sha": last_sha,
                "new_sha": latest["sha"],
                "message": latest["message"],
                "author": latest["author"],
                "date": latest["date"]
            })
            state[key] = latest["sha"]

    if has_updates:
        print("UPDATE_FOUND")
        print(json.dumps(report, indent=2))
        with open(STATE_FILE, "w") as f:
            json.dump(state, f, indent=2)
    else:
        print("NO_UPDATES")
        print(f"Ambos repositorios están sincronizados con los últimos commits conocidos:\n- soundcraft: {state.get('soundcraft')[:8]}\n- prowave: {state.get('prowave')[:8]}")

if __name__ == "__main__":
    main()
