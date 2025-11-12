import json
import inquirer

with open("repositories.json", "r", encoding="utf-8") as f:
    repositories = json.load(f)
with open("results.json", "r", encoding="utf-8") as f:
    results = json.load(f)

for i, repo in enumerate(repositories, start=1):
    result = results.get(repo["full_name"], {})
    
    if result.get("status") == "success":
        continue

    print(f"[{i}/{len(repositories)}] {repo['full_name']} (stars: {repo['stargazers_count']})")
    print(f"URL: {repo['html_url']}")
    print(f"Issue/PR 1: {repo['html_url']}/issues/1")
    print(f"Issue/PR 10: {repo['html_url']}/issues/10")
    print(f"Issue/PR 11: {repo['html_url']}/issues/11")
    print(f"Issue/PR 12: {repo['html_url']}/issues/12")
    print(f"Issue/PR 13: {repo['html_url']}/issues/13")
    print(f"Description: {repo['description']}")

    questions = [
        inquirer.List(
            "migrated",
            message=f"Select migration status",
            choices=["yes/maybe", "no", "skip"],
            default="no"
        )
    ]
    answers = inquirer.prompt(questions)

    if answers is None or answers["migrated"] == "exit":
        break
    if answers["migrated"] == "skip":
        continue
    
    result["migrated"] = answers["migrated"] == "yes/maybe"
    result["status"] = "success"

    results[repo["full_name"]] = result

with open("results.json", "w", encoding="utf-8") as f:
    json.dump(results, f, indent=4)