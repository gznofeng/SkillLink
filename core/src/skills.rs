use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub dir_name: String,
    pub name: String,
    pub description: String,
}

/// 扫描中央仓库：每个含 SKILL.md 的直接子目录 = 一个 skill。
/// 仓库不存在/为空 → 空列表；仅扫描一层。
pub fn list_skills(central: &str) -> Vec<SkillInfo> {
    let root = Path::new(central);
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut skills: Vec<SkillInfo> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| e.path().join("SKILL.md").is_file())
        .filter_map(|e| {
            let dir_name = e.file_name().to_string_lossy().to_string();
            let raw = std::fs::read_to_string(e.path().join("SKILL.md")).ok()?;
            let (name, description) = parse_frontmatter(&raw);
            Some(SkillInfo {
                name: name.unwrap_or_else(|| dir_name.clone()),
                description,
                dir_name,
            })
        })
        .collect();
    skills.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
    skills
}

/// 解析 SKILL.md 头部 YAML frontmatter 的 name / description。
/// frontmatter 缺失 → (None, "")；字段缺失 → 兜底。
fn parse_frontmatter(raw: &str) -> (Option<String>, String) {
    let mut lines = raw.lines();
    if lines.next().map(|l| l.trim()) != Some("---") {
        return (None, String::new());
    }
    let mut name = None;
    let mut description = String::new();
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed.trim() == "---" {
            break;
        }
        if let Some(v) = trimmed.strip_prefix("name:") {
            name = Some(v.trim().trim_matches(|c| c == '"' || c == '\'').to_string());
        } else if let Some(v) = trimmed.strip_prefix("description:") {
            description = v.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
        }
    }
    (name, description)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn temp_repo(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "skill-link-scan-{tag}-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    use std::path::PathBuf;

    fn make_skill(root: &Path, dir: &str, skill_md: Option<&str>) {
        let p = root.join(dir);
        std::fs::create_dir_all(&p).unwrap();
        if let Some(content) = skill_md {
            std::fs::write(p.join("SKILL.md"), content).unwrap();
        }
    }

    #[test]
    fn should_list_skills_with_frontmatter() {
        let repo = temp_repo("fm");
        make_skill(&repo, "commit", Some("---\nname: commit-std\ndescription: 提交规范\n---\n\n正文"));
        make_skill(&repo, "review", Some("---\nname: review-std\ndescription: 评审规范\n---\n"));
        let skills = list_skills(repo.to_str().unwrap());
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].dir_name, "commit");
        assert_eq!(skills[0].name, "commit-std");
        assert_eq!(skills[0].description, "提交规范");
        assert_eq!(skills[1].name, "review-std");
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn should_skip_dirs_without_skill_md() {
        let repo = temp_repo("skip");
        make_skill(&repo, "real", Some("---\nname: real\n---\n"));
        make_skill(&repo, "notes", None);
        let skills = list_skills(repo.to_str().unwrap());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].dir_name, "real");
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn should_return_empty_when_central_missing() {
        let skills = list_skills("Z:/definitely-not-exist-skill-link");
        assert!(skills.is_empty());
    }

    #[test]
    fn should_fallback_name_when_frontmatter_missing() {
        let repo = temp_repo("nofm");
        make_skill(&repo, "plain", Some("# Just markdown\n\nno frontmatter"));
        let skills = list_skills(repo.to_str().unwrap());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "plain");
        assert_eq!(skills[0].description, "");
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn should_parse_description_with_special_chars() {
        let repo = temp_repo("special");
        make_skill(
            &repo,
            "weird",
            Some("---\nname: weird\ndescription: 提交: 必须遵循 Angular 规范（含冒号）\n---\n"),
        );
        let skills = list_skills(repo.to_str().unwrap());
        assert_eq!(skills[0].description, "提交: 必须遵循 Angular 规范（含冒号）");
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn should_return_empty_list_for_empty_repo() {
        let repo = temp_repo("empty");
        let skills = list_skills(repo.to_str().unwrap());
        assert!(skills.is_empty());
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn should_not_recurse_into_nested_dirs() {
        let repo = temp_repo("nested");
        make_skill(&repo, "top", Some("---\nname: top\n---\n"));
        let nested = repo.join("top").join("sub");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("SKILL.md"), "---\nname: hidden\n---\n").unwrap();
        let skills = list_skills(repo.to_str().unwrap());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].dir_name, "top");
        std::fs::remove_dir_all(&repo).ok();
    }
}
