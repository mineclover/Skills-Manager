use std::fs;
use std::path::Path;

fn plugin_header_matches(header: &str, skill_id: &str) -> bool {
    let Some(plugin_id) = header
        .strip_prefix("[plugins.\"")
        .and_then(|value| value.strip_suffix("\"]"))
    else {
        return false;
    };

    plugin_id == skill_id
        || plugin_id
            .strip_prefix(skill_id)
            .is_some_and(|suffix| suffix.starts_with('@'))
}

fn is_section_header(line: &str) -> bool {
    line.starts_with('[') && line.ends_with(']')
}

fn enabled_key_value(line: &str) -> Option<&str> {
    let (key, value) = line.split_once('=')?;
    (key.trim() == "enabled").then_some(value.trim())
}

fn parse_bool_value(value: &str) -> Option<bool> {
    value
        .split('#')
        .next()
        .map(str::trim)
        .and_then(|value| value.parse::<bool>().ok())
}

fn line_ending(content: &str) -> &str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn insert_enabled_line(output: &mut Vec<String>, enabled: bool) {
    let insertion_index = output
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map(|index| index + 1)
        .unwrap_or(output.len());
    output.insert(insertion_index, format!("enabled = {enabled}"));
}

pub fn plugin_enabled(config_path: &Path, skill_id: &str) -> bool {
    let config_toml_path = config_path.join("config.toml");
    if !config_toml_path.exists() {
        return true;
    }

    let content = match fs::read_to_string(&config_toml_path) {
        Ok(content) => content,
        Err(_) => return true,
    };

    let mut in_target_plugin = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if is_section_header(trimmed) {
            in_target_plugin = plugin_header_matches(trimmed, skill_id);
        }

        if in_target_plugin {
            if let Some(value) = enabled_key_value(trimmed) {
                return parse_bool_value(value).unwrap_or(true);
            }
        }
    }

    true
}

pub fn set_plugin_enabled(config_path: &Path, skill_id: &str, enabled: bool) -> Result<(), String> {
    let config_toml_path = config_path.join("config.toml");
    if !config_toml_path.exists() {
        fs::create_dir_all(config_path)
            .map_err(|error| format!("Failed to create codex config directory: {error}"))?;
        let content = format!("[plugins.\"{skill_id}\"]\nenabled = {enabled}\n");
        fs::write(&config_toml_path, content)
            .map_err(|error| format!("Failed to write codex config.toml: {error}"))?;
        return Ok(());
    }

    let content = fs::read_to_string(&config_toml_path)
        .map_err(|error| format!("Failed to read codex config.toml: {error}"))?;
    let newline = line_ending(&content);
    let had_trailing_newline = content.ends_with('\n');
    let mut output = Vec::new();
    let mut in_target_plugin = false;
    let mut found_target_plugin = false;
    let mut wrote_enabled = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if is_section_header(trimmed) {
            if in_target_plugin && !wrote_enabled {
                insert_enabled_line(&mut output, enabled);
                wrote_enabled = true;
            }

            in_target_plugin = plugin_header_matches(trimmed, skill_id);
            found_target_plugin |= in_target_plugin;
        }

        if in_target_plugin {
            if enabled_key_value(trimmed).is_some() {
                let indent: String = line
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .collect();
                output.push(format!("{indent}enabled = {enabled}"));
                wrote_enabled = true;
                continue;
            }
        }

        output.push(line.to_string());
    }

    if in_target_plugin && !wrote_enabled {
        insert_enabled_line(&mut output, enabled);
    }

    if !found_target_plugin {
        if !output.is_empty() {
            output.push(String::new());
        }
        output.push(format!("[plugins.\"{skill_id}\"]"));
        output.push(format!("enabled = {enabled}"));
    }

    let mut new_content = output.join(newline);
    if had_trailing_newline {
        new_content.push_str(newline);
    }

    fs::write(&config_toml_path, new_content)
        .map_err(|error| format!("Failed to write codex config.toml: {error}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{plugin_enabled, set_plugin_enabled};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_config_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("skills-manager-codex-config-{suffix}"));
        fs::create_dir_all(&path).expect("create temp config directory");
        path
    }

    #[test]
    fn plugin_id_matching_does_not_accept_longer_prefixes() {
        let config_dir = temp_config_dir();
        fs::write(
            config_dir.join("config.toml"),
            "[plugins.\"imagegen-extra\"]\nenabled = false\n[plugins.\"imagegen@runtime\"]\nenabled = true\n",
        )
        .expect("write config");

        assert!(plugin_enabled(&config_dir, "imagegen"));
        assert!(!plugin_enabled(&config_dir, "imagegen-extra"));
        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn setting_existing_plugin_preserves_other_sections_and_line_endings() {
        let config_dir = temp_config_dir();
        fs::write(
            config_dir.join("config.toml"),
            "[plugins.\"imagegen@runtime\"]\r\nname = \"Image\"\r\n\r\n[other]\r\nenabled = true\r\n",
        )
        .expect("write config");

        set_plugin_enabled(&config_dir, "imagegen", false).expect("set plugin state");
        let updated = fs::read_to_string(config_dir.join("config.toml")).expect("read config");
        assert!(updated.contains("enabled = false\r\n\r\n[other]"));
        assert!(updated.contains("enabled = true\r\n"));
        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn setting_missing_plugin_appends_a_new_section() {
        let config_dir = temp_config_dir();
        fs::write(config_dir.join("config.toml"), "[other]\nenabled = true\n")
            .expect("write config");

        set_plugin_enabled(&config_dir, "imagegen", false).expect("set plugin state");
        let updated = fs::read_to_string(config_dir.join("config.toml")).expect("read config");
        assert!(updated.contains("[plugins.\"imagegen\"]\nenabled = false"));
        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn setting_plugin_creates_missing_config_file() {
        let config_dir = temp_config_dir();

        set_plugin_enabled(&config_dir, "imagegen", false).expect("set plugin state");
        let updated = fs::read_to_string(config_dir.join("config.toml")).expect("read config");
        assert_eq!(updated, "[plugins.\"imagegen\"]\nenabled = false\n");
        let _ = fs::remove_dir_all(config_dir);
    }
}
