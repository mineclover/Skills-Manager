use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;

use serde::Serialize;
use tmpskills_manager_temp_lib::models::config::SkillActivationPreset;
use tmpskills_manager_temp_lib::models::{
    builtin_skill_activation_presets, home_dir, AppConfig, InstalledSkillPackage, ProjectBinding,
    SaveLocalSkillContractRequest, Skill, SkillBinding, SkillContract, SkillContractSummary,
    SkillOperationPreview, SkillOperationReport, SkillProviderInventory, SkillScope,
    SUPPORTED_TOOLS,
};
use tmpskills_manager_temp_lib::services::{
    BatchSetSkillToolsRequest, BatchSetSkillToolsResponse, BatchSkillToolAction,
    BatchSkillToolTarget, BatchSkillToolTargetKind, ProviderInventoryService, ScannerService,
    SkillControlService, SkillPackageService, ToolControlService, WorkspaceService,
};

#[derive(Debug, Default)]
struct InspectOptions {
    project_id: Option<String>,
    json: bool,
}

#[derive(Debug)]
enum Command {
    Inspect(InspectOptions),
    Projects {
        json: bool,
    },
    PreviewProject {
        path: String,
        name: Option<String>,
        json: bool,
    },
    AddProject {
        path: String,
        name: Option<String>,
        json: bool,
    },
    UseProject {
        project_id: String,
        json: bool,
    },
    RemoveProject {
        project_id: String,
        json: bool,
    },
    Providers {
        project_id: Option<String>,
        json: bool,
    },
    Bindings {
        project_id: Option<String>,
        provider_id: Option<String>,
        skill_instance_id: Option<String>,
        json: bool,
    },
    PreviewSkill {
        instance_id: String,
        tool_id: String,
        project_id: Option<String>,
        enabled: bool,
        json: bool,
    },
    SetSkill {
        instance_id: String,
        tool_id: String,
        project_id: Option<String>,
        enabled: bool,
        confirm_shared: bool,
        json: bool,
    },
    CreateSkill {
        name: String,
        description: Option<String>,
        json: bool,
    },
    DeleteSkill {
        instance_id: String,
        json: bool,
    },
    ImportSkills {
        paths: Vec<String>,
        json: bool,
    },
    SetLocalSkillContract {
        instance_id: String,
        file: String,
        json: bool,
    },
    SetTool {
        tool_id: String,
        enabled: bool,
        json: bool,
    },
    BatchSet {
        request: BatchSetSkillToolsRequest,
        json: bool,
    },
    ApplyPreset {
        preset_id: String,
        project_id: Option<String>,
        tool_id: Option<String>,
        json: bool,
    },
    CreatePreset {
        name: String,
        description: Option<String>,
        copy_current_state: bool,
        project_id: Option<String>,
        tool_id: Option<String>,
        json: bool,
    },
    DeletePreset {
        preset_id: String,
        json: bool,
    },
    CapturePreset {
        preset_id: String,
        project_id: Option<String>,
        tool_id: String,
        json: bool,
    },
    SetPresetSkill {
        preset_id: String,
        project_id: Option<String>,
        tool_id: String,
        skill_id: String,
        enabled: bool,
        json: bool,
    },
    SetPresetAll {
        preset_id: String,
        project_id: Option<String>,
        tool_id: String,
        enabled: bool,
        json: bool,
    },
    ClearPreset {
        json: bool,
    },
}

#[derive(Debug, Serialize)]
struct InspectReport {
    scope: String,
    skills_dir: String,
    active_project_id: Option<String>,
    projects: Vec<ProjectSummary>,
    counts: CountSummary,
    skills: Vec<Skill>,
    tools: Vec<ToolSummary>,
    packages: Vec<InstalledSkillPackage>,
    presets: Vec<SkillActivationPreset>,
}

#[derive(Debug, Serialize)]
struct ProjectSummary {
    id: String,
    name: String,
    skills_dir: String,
    root_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProjectListReport {
    active_project_id: Option<String>,
    projects: Vec<ProjectSummary>,
}

#[derive(Debug, Serialize)]
struct CountSummary {
    total: usize,
    managed: usize,
    tool: usize,
    packages: usize,
}

#[derive(Debug, Serialize)]
struct ToolSummary {
    id: String,
    name: Option<String>,
    detected: bool,
    enabled: bool,
    skills_path: String,
    direct_skill_count: usize,
    direct_skill_ids: Vec<String>,
}

fn usage() {
    println!(
        "Usage:\n\
         skills-manager-inspect [inspect] [--scope global|project:<id>] [--json]\n\
         skills-manager-inspect project list [--json]\n\
         skills-manager-inspect project preview --path <directory> [--name <name>] [--json]\n\
         skills-manager-inspect project add --path <directory> [--name <name>] [--json]\n\
         skills-manager-inspect project use --id <project-id> [--json]\n\
         skills-manager-inspect project remove --id <project-id> [--json]\n\
         skills-manager-inspect providers [--project <id>] [--json]\n\
         skills-manager-inspect bindings [--project <id>] [--provider <id>] [--skill <instance-id>] [--json]\n\
         skills-manager-inspect binding list [--project <id>] [--provider <id>] [--skill <instance-id>] [--json]\n\
         skills-manager-inspect skill preview --id <instance-id> --tool <tool-id> --enable|--disable [--project <id>] [--json]\n\
         skills-manager-inspect skill list [--scope global|project:<id>] [--json]\n\
         skills-manager-inspect skill enable --id <instance-id> --tool <tool-id> [--project <id>] [--confirm-shared] [--json]\n\
         skills-manager-inspect skill disable --id <instance-id> --tool <tool-id> [--project <id>] [--confirm-shared] [--json]\n\
         skills-manager-inspect skill create --name <name> [--description <text>] [--json]\n\
         skills-manager-inspect skill delete --id <instance-id> [--json]\n\
         skills-manager-inspect skill import --path <directory> [--path <directory>] [--json]\n\
         skills-manager-inspect skill contract set --id <instance-id> --file <contract.yaml> [--json]\n\
         skills-manager-inspect tool enable --id <tool-id> [--json]\n\
         skills-manager-inspect tool disable --id <tool-id> [--json]\n\
         skills-manager-inspect batch enable --skill <instance-id> --tool <tool-id> [--json]\n\
         skills-manager-inspect batch disable --group <group-id> --tool <tool-id> [--json]\n\
         skills-manager-inspect preset apply --id <preset-id> [--project <id>] [--tool <id>] [--json]\n\
         skills-manager-inspect preset create --name <name> [--description <text>] [--copy-current] [--project <id>] [--tool <id>] [--json]\n\
         skills-manager-inspect preset delete --id <preset-id> [--json]\n\
         skills-manager-inspect preset capture --id <preset-id> --tool <tool-id> [--project <id>] [--json]\n\
         skills-manager-inspect preset skill enable --id <preset-id> --skill <instance-id> --tool <tool-id> [--project <id>] [--json]\n\
         skills-manager-inspect preset skill disable --id <preset-id> --skill <instance-id> --tool <tool-id> [--project <id>] [--json]\n\
         skills-manager-inspect preset all enable --id <preset-id> --tool <tool-id> [--project <id>] [--json]\n\
         skills-manager-inspect preset all disable --id <preset-id> --tool <tool-id> [--project <id>] [--json]\n\
         skills-manager-inspect preset list [--scope global|project:<id>] [--json]\n\
         skills-manager-inspect preset clear [--json]\n\n\
         Inspect uses the same scanner as the UI. Skill, contract, and preset mutations use\n\
         the same shared Rust control service as the Tauri commands."
    );
}

fn parse_scope_options(args: &[String]) -> Result<InspectOptions, String> {
    let mut options = InspectOptions::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--help" | "-h" => {
                usage();
                process::exit(0);
            }
            "--json" => options.json = true,
            "--project" => {
                index += 1;
                let project_id = args
                    .get(index)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| "--project requires a project id".to_string())?;
                options.project_id = Some(project_id.clone());
            }
            "--scope" => {
                index += 1;
                let scope = args
                    .get(index)
                    .ok_or_else(|| "--scope requires global or project:<id>".to_string())?;
                options.project_id = match scope.as_str() {
                    "global" => None,
                    value if value.starts_with("project:") => {
                        let project_id = value.trim_start_matches("project:").trim();
                        if project_id.is_empty() {
                            return Err("project scope requires a project id".to_string());
                        }
                        Some(project_id.to_string())
                    }
                    _ => {
                        return Err("invalid scope; use global or project:<project-id>".to_string())
                    }
                };
            }
            value => return Err(format!("unknown argument: {value}")),
        }
        index += 1;
    }

    Ok(options)
}

fn required_option(args: &[String], index: &mut usize, name: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn parse_command() -> Result<Command, String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Ok(Command::Inspect(InspectOptions::default()));
    }

    match args[0].as_str() {
        "--help" | "-h" => {
            usage();
            process::exit(0);
        }
        "inspect" => Ok(Command::Inspect(parse_scope_options(&args[1..])?)),
        "project" | "projects" => {
            let action = if args[0] == "projects" {
                "list"
            } else {
                args.get(1).map(String::as_str).ok_or_else(|| {
                    "project requires list, preview, add, use, or remove".to_string()
                })?
            };
            let offset = if args[0] == "projects" { 1 } else { 2 };

            match action {
                "list" => {
                    let json = args[offset..].iter().all(|value| value == "--json");
                    if args[offset..].iter().any(|value| value != "--json") {
                        return Err("unknown project list option".to_string());
                    }
                    Ok(Command::Projects { json })
                }
                "preview" | "add" => {
                    let mut path = None;
                    let mut name = None;
                    let mut json = false;
                    let mut index = offset;
                    while index < args.len() {
                        match args[index].as_str() {
                            "--path" => path = Some(required_option(&args, &mut index, "--path")?),
                            "--name" => name = Some(required_option(&args, &mut index, "--name")?),
                            "--json" => json = true,
                            value => return Err(format!("unknown project option: {value}")),
                        }
                        index += 1;
                    }
                    let path = path.ok_or_else(|| format!("project {action} requires --path"))?;
                    if action == "preview" {
                        Ok(Command::PreviewProject { path, name, json })
                    } else {
                        Ok(Command::AddProject { path, name, json })
                    }
                }
                "use" | "remove" => {
                    let mut project_id = None;
                    let mut json = false;
                    let mut index = offset;
                    while index < args.len() {
                        match args[index].as_str() {
                            "--id" => {
                                project_id = Some(required_option(&args, &mut index, "--id")?)
                            }
                            "--json" => json = true,
                            value => return Err(format!("unknown project option: {value}")),
                        }
                        index += 1;
                    }
                    let project_id =
                        project_id.ok_or_else(|| format!("project {action} requires --id"))?;
                    if action == "use" {
                        Ok(Command::UseProject { project_id, json })
                    } else {
                        Ok(Command::RemoveProject { project_id, json })
                    }
                }
                value => Err(format!("unknown project action: {value}")),
            }
        }
        "providers" | "provider" => {
            let offset = if args[0] == "provider" {
                if args.get(1).map(String::as_str) != Some("list") {
                    return Err("provider requires list".to_string());
                }
                2
            } else {
                1
            };
            let mut project_id = None;
            let mut json = false;
            let mut index = offset;
            while index < args.len() {
                match args[index].as_str() {
                    "--project" => {
                        project_id = Some(required_option(&args, &mut index, "--project")?)
                    }
                    "--json" => json = true,
                    value => return Err(format!("unknown provider option: {value}")),
                }
                index += 1;
            }
            Ok(Command::Providers { project_id, json })
        }
        "bindings" | "binding" => {
            let offset = if args[0] == "binding" {
                if args.get(1).map(String::as_str) != Some("list") {
                    return Err("binding requires list".to_string());
                }
                2
            } else {
                1
            };
            let mut project_id = None;
            let mut provider_id = None;
            let mut skill_instance_id = None;
            let mut json = false;
            let mut index = offset;
            while index < args.len() {
                match args[index].as_str() {
                    "--project" => {
                        project_id = Some(required_option(&args, &mut index, "--project")?)
                    }
                    "--provider" => {
                        provider_id = Some(required_option(&args, &mut index, "--provider")?)
                    }
                    "--skill" => {
                        skill_instance_id = Some(required_option(&args, &mut index, "--skill")?)
                    }
                    "--json" => json = true,
                    value => return Err(format!("unknown binding option: {value}")),
                }
                index += 1;
            }
            Ok(Command::Bindings {
                project_id,
                provider_id,
                skill_instance_id,
                json,
            })
        }
        "skill" => {
            let action = args
                .get(1)
                .ok_or_else(|| "skill requires list, preview, enable, or disable".to_string())?;
            if action == "list" {
                return Ok(Command::Inspect(parse_scope_options(&args[2..])?));
            }

            if action == "contract" {
                if args.get(2).map(String::as_str) != Some("set") {
                    return Err("skill contract requires set".to_string());
                }
                let mut instance_id = None;
                let mut file = None;
                let mut json = false;
                let mut index = 3;
                while index < args.len() {
                    match args[index].as_str() {
                        "--id" => instance_id = Some(required_option(&args, &mut index, "--id")?),
                        "--file" => file = Some(required_option(&args, &mut index, "--file")?),
                        "--json" => json = true,
                        value => return Err(format!("unknown skill contract option: {value}")),
                    }
                    index += 1;
                }
                return Ok(Command::SetLocalSkillContract {
                    instance_id: instance_id
                        .ok_or_else(|| "skill contract set requires --id".to_string())?,
                    file: file.ok_or_else(|| "skill contract set requires --file".to_string())?,
                    json,
                });
            }

            if action == "preview" {
                let mut instance_id = None;
                let mut tool_id = None;
                let mut project_id = None;
                let mut enabled = None;
                let mut json = false;
                let mut index = 2;
                while index < args.len() {
                    match args[index].as_str() {
                        "--id" => instance_id = Some(required_option(&args, &mut index, "--id")?),
                        "--tool" => tool_id = Some(required_option(&args, &mut index, "--tool")?),
                        "--project" => {
                            project_id = Some(required_option(&args, &mut index, "--project")?)
                        }
                        "--enable" => enabled = Some(true),
                        "--disable" => enabled = Some(false),
                        "--json" => json = true,
                        value => return Err(format!("unknown skill preview option: {value}")),
                    }
                    index += 1;
                }
                return Ok(Command::PreviewSkill {
                    instance_id: instance_id
                        .ok_or_else(|| "skill preview requires --id".to_string())?,
                    tool_id: tool_id.ok_or_else(|| "skill preview requires --tool".to_string())?,
                    project_id,
                    enabled: enabled.ok_or_else(|| {
                        "skill preview requires --enable or --disable".to_string()
                    })?,
                    json,
                });
            }

            if action == "create" {
                let mut name = None;
                let mut description = None;
                let mut json = false;
                let mut index = 2;
                while index < args.len() {
                    match args[index].as_str() {
                        "--name" => name = Some(required_option(&args, &mut index, "--name")?),
                        "--description" => {
                            description = Some(required_option(&args, &mut index, "--description")?)
                        }
                        "--json" => json = true,
                        value => return Err(format!("unknown skill option: {value}")),
                    }
                    index += 1;
                }
                return Ok(Command::CreateSkill {
                    name: name.ok_or_else(|| "skill create requires --name".to_string())?,
                    description,
                    json,
                });
            }

            if action == "delete" {
                let mut instance_id = None;
                let mut json = false;
                let mut index = 2;
                while index < args.len() {
                    match args[index].as_str() {
                        "--id" => instance_id = Some(required_option(&args, &mut index, "--id")?),
                        "--json" => json = true,
                        value => return Err(format!("unknown skill option: {value}")),
                    }
                    index += 1;
                }
                return Ok(Command::DeleteSkill {
                    instance_id: instance_id
                        .ok_or_else(|| "skill delete requires --id".to_string())?,
                    json,
                });
            }

            if action == "import" {
                let mut paths = Vec::new();
                let mut json = false;
                let mut index = 2;
                while index < args.len() {
                    match args[index].as_str() {
                        "--path" => paths.push(required_option(&args, &mut index, "--path")?),
                        "--json" => json = true,
                        value => return Err(format!("unknown skill option: {value}")),
                    }
                    index += 1;
                }
                if paths.is_empty() {
                    return Err("skill import requires at least one --path".to_string());
                }
                return Ok(Command::ImportSkills { paths, json });
            }

            let enabled = match action.as_str() {
                "enable" => true,
                "disable" => false,
                _ => return Err(format!("unknown skill action: {action}")),
            };
            let mut instance_id = None;
            let mut tool_id = None;
            let mut project_id = None;
            let mut confirm_shared = false;
            let mut json = false;
            let mut index = 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--id" => instance_id = Some(required_option(&args, &mut index, "--id")?),
                    "--tool" => tool_id = Some(required_option(&args, &mut index, "--tool")?),
                    "--project" => {
                        project_id = Some(required_option(&args, &mut index, "--project")?)
                    }
                    "--confirm-shared" => confirm_shared = true,
                    "--json" => json = true,
                    value => return Err(format!("unknown skill option: {value}")),
                }
                index += 1;
            }
            Ok(Command::SetSkill {
                instance_id: instance_id.ok_or_else(|| "skill requires --id".to_string())?,
                tool_id: tool_id.ok_or_else(|| "skill requires --tool".to_string())?,
                project_id,
                enabled,
                confirm_shared,
                json,
            })
        }
        "tool" => {
            let action = args
                .get(1)
                .ok_or_else(|| "tool requires enable or disable".to_string())?;
            let enabled = match action.as_str() {
                "enable" => true,
                "disable" => false,
                _ => return Err(format!("unknown tool action: {action}")),
            };
            let mut tool_id = None;
            let mut json = false;
            let mut index = 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--id" => tool_id = Some(required_option(&args, &mut index, "--id")?),
                    "--json" => json = true,
                    value => return Err(format!("unknown tool option: {value}")),
                }
                index += 1;
            }
            Ok(Command::SetTool {
                tool_id: tool_id.ok_or_else(|| "tool requires --id".to_string())?,
                enabled,
                json,
            })
        }
        "preset" => {
            let action = args.get(1).ok_or_else(|| {
                "preset requires apply, create, delete, capture, skill, all, list, or clear"
                    .to_string()
            })?;
            match action.as_str() {
                "list" => Ok(Command::Inspect(parse_scope_options(&args[2..])?)),
                "clear" => {
                    let json = args[2..].iter().any(|arg| arg == "--json");
                    if args[2..].iter().any(|arg| arg != "--json") {
                        return Err("unknown preset clear option".to_string());
                    }
                    Ok(Command::ClearPreset { json })
                }
                "apply" => {
                    let mut preset_id = None;
                    let mut project_id = None;
                    let mut tool_id = None;
                    let mut json = false;
                    let mut index = 2;
                    while index < args.len() {
                        match args[index].as_str() {
                            "--id" => preset_id = Some(required_option(&args, &mut index, "--id")?),
                            "--project" => {
                                project_id = Some(required_option(&args, &mut index, "--project")?)
                            }
                            "--tool" => {
                                tool_id = Some(required_option(&args, &mut index, "--tool")?)
                            }
                            "--json" => json = true,
                            value => return Err(format!("unknown preset option: {value}")),
                        }
                        index += 1;
                    }
                    Ok(Command::ApplyPreset {
                        preset_id: preset_id
                            .ok_or_else(|| "preset apply requires --id".to_string())?,
                        project_id,
                        tool_id,
                        json,
                    })
                }
                "create" => {
                    let mut name = None;
                    let mut description = None;
                    let mut copy_current_state = false;
                    let mut project_id = None;
                    let mut tool_id = None;
                    let mut json = false;
                    let mut index = 2;
                    while index < args.len() {
                        match args[index].as_str() {
                            "--name" => name = Some(required_option(&args, &mut index, "--name")?),
                            "--description" => {
                                description =
                                    Some(required_option(&args, &mut index, "--description")?)
                            }
                            "--copy-current" => copy_current_state = true,
                            "--project" => {
                                project_id = Some(required_option(&args, &mut index, "--project")?)
                            }
                            "--tool" => {
                                tool_id = Some(required_option(&args, &mut index, "--tool")?)
                            }
                            "--json" => json = true,
                            value => return Err(format!("unknown preset create option: {value}")),
                        }
                        index += 1;
                    }
                    Ok(Command::CreatePreset {
                        name: name.ok_or_else(|| "preset create requires --name".to_string())?,
                        description,
                        copy_current_state,
                        project_id,
                        tool_id,
                        json,
                    })
                }
                "delete" => {
                    let mut preset_id = None;
                    let mut json = false;
                    let mut index = 2;
                    while index < args.len() {
                        match args[index].as_str() {
                            "--id" => preset_id = Some(required_option(&args, &mut index, "--id")?),
                            "--json" => json = true,
                            value => return Err(format!("unknown preset delete option: {value}")),
                        }
                        index += 1;
                    }
                    Ok(Command::DeletePreset {
                        preset_id: preset_id
                            .ok_or_else(|| "preset delete requires --id".to_string())?,
                        json,
                    })
                }
                "capture" => {
                    let mut preset_id = None;
                    let mut project_id = None;
                    let mut tool_id = None;
                    let mut json = false;
                    let mut index = 2;
                    while index < args.len() {
                        match args[index].as_str() {
                            "--id" => preset_id = Some(required_option(&args, &mut index, "--id")?),
                            "--project" => {
                                project_id = Some(required_option(&args, &mut index, "--project")?)
                            }
                            "--tool" => {
                                tool_id = Some(required_option(&args, &mut index, "--tool")?)
                            }
                            "--json" => json = true,
                            value => return Err(format!("unknown preset capture option: {value}")),
                        }
                        index += 1;
                    }
                    Ok(Command::CapturePreset {
                        preset_id: preset_id
                            .ok_or_else(|| "preset capture requires --id".to_string())?,
                        project_id,
                        tool_id: tool_id
                            .ok_or_else(|| "preset capture requires --tool".to_string())?,
                        json,
                    })
                }
                "skill" | "all" => {
                    let kind = action.as_str();
                    let sub_action = args
                        .get(2)
                        .ok_or_else(|| format!("preset {kind} requires enable or disable"))?;
                    let enabled = match sub_action.as_str() {
                        "enable" => true,
                        "disable" => false,
                        value => return Err(format!("unknown preset {kind} action: {value}")),
                    };
                    let mut preset_id = None;
                    let mut project_id = None;
                    let mut tool_id = None;
                    let mut skill_id = None;
                    let mut json = false;
                    let mut index = 3;
                    while index < args.len() {
                        match args[index].as_str() {
                            "--id" => preset_id = Some(required_option(&args, &mut index, "--id")?),
                            "--project" => {
                                project_id = Some(required_option(&args, &mut index, "--project")?)
                            }
                            "--tool" => {
                                tool_id = Some(required_option(&args, &mut index, "--tool")?)
                            }
                            "--skill" => {
                                skill_id = Some(required_option(&args, &mut index, "--skill")?)
                            }
                            "--json" => json = true,
                            value => return Err(format!("unknown preset {kind} option: {value}")),
                        }
                        index += 1;
                    }
                    if kind == "skill" {
                        Ok(Command::SetPresetSkill {
                            preset_id: preset_id
                                .ok_or_else(|| "preset skill requires --id".to_string())?,
                            project_id,
                            tool_id: tool_id
                                .ok_or_else(|| "preset skill requires --tool".to_string())?,
                            skill_id: skill_id
                                .ok_or_else(|| "preset skill requires --skill".to_string())?,
                            enabled,
                            json,
                        })
                    } else {
                        if skill_id.is_some() {
                            return Err("preset all does not accept --skill".to_string());
                        }
                        Ok(Command::SetPresetAll {
                            preset_id: preset_id
                                .ok_or_else(|| "preset all requires --id".to_string())?,
                            project_id,
                            tool_id: tool_id
                                .ok_or_else(|| "preset all requires --tool".to_string())?,
                            enabled,
                            json,
                        })
                    }
                }
                _ => Err(format!("unknown preset action: {action}")),
            }
        }
        "batch" => {
            let action = args
                .get(1)
                .ok_or_else(|| "batch requires enable or disable".to_string())?;
            let action = match action.as_str() {
                "enable" => BatchSkillToolAction::Enable,
                "disable" => BatchSkillToolAction::Disable,
                _ => return Err(format!("unknown batch action: {action}")),
            };
            let mut targets = Vec::new();
            let mut tool_ids = Vec::new();
            let mut json = false;
            let mut index = 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--skill" => targets.push(BatchSkillToolTarget {
                        kind: BatchSkillToolTargetKind::Skill,
                        id: required_option(&args, &mut index, "--skill")?,
                    }),
                    "--group" => targets.push(BatchSkillToolTarget {
                        kind: BatchSkillToolTargetKind::Group,
                        id: required_option(&args, &mut index, "--group")?,
                    }),
                    "--tool" => tool_ids.push(required_option(&args, &mut index, "--tool")?),
                    "--json" => json = true,
                    value => return Err(format!("unknown batch option: {value}")),
                }
                index += 1;
            }
            if targets.is_empty() {
                return Err("batch requires at least one --skill or --group".to_string());
            }
            if tool_ids.is_empty() {
                return Err("batch requires at least one --tool".to_string());
            }
            Ok(Command::BatchSet {
                request: BatchSetSkillToolsRequest {
                    targets,
                    tool_ids,
                    action,
                },
                json,
            })
        }
        value if value.starts_with('-') => Ok(Command::Inspect(parse_scope_options(&args)?)),
        value => Err(format!("unknown command: {value}")),
    }
}

fn inspect(options: InspectOptions) -> Result<InspectReport, String> {
    // Inspect must not trigger ConfigManager's migration/default-save path.
    // Read the persisted config directly so this command remains read-only.
    let config = load_persisted_config()?;
    let project_id = options.project_id.as_deref();
    let scope = project_id
        .map(|id| format!("project:{id}"))
        .unwrap_or_else(|| "global".to_string());
    let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
    let packages = SkillPackageService::list_discovered_packages(&config.skills_dir)?;

    let managed = skills
        .iter()
        .filter(|skill| skill.scope != SkillScope::Tool)
        .count();
    let tool = skills.len() - managed;

    let mut names = SUPPORTED_TOOLS
        .iter()
        .map(|tool| (tool.id.to_string(), tool.name.to_string()))
        .collect::<HashMap<_, _>>();
    names.extend(
        config
            .custom_tools
            .iter()
            .map(|(id, tool)| (id.clone(), tool.name.clone())),
    );
    let direct_by_tool = skills
        .iter()
        .filter(|skill| skill.scope == SkillScope::Tool)
        .fold(
            HashMap::<String, Vec<String>>::new(),
            |mut result, skill| {
                if let Some(tool_id) = &skill.tool_id {
                    result
                        .entry(tool_id.clone())
                        .or_default()
                        .push(skill.id.clone());
                }
                result
            },
        );

    let mut tools = config
        .collect_tool_configs()
        .into_iter()
        .filter_map(|(id, tool_config)| {
            let mut direct_skill_ids = direct_by_tool.get(&id).cloned().unwrap_or_default();
            direct_skill_ids.sort();
            if !tool_config.detected && !tool_config.enabled && direct_skill_ids.is_empty() {
                return None;
            }
            Some(ToolSummary {
                name: names.get(&id).cloned(),
                id,
                detected: tool_config.detected,
                enabled: tool_config.enabled,
                skills_path: tool_config.skills_path.to_string_lossy().into_owned(),
                direct_skill_count: direct_skill_ids.len(),
                direct_skill_ids,
            })
        })
        .collect::<Vec<_>>();
    tools.sort_by(|a, b| a.id.cmp(&b.id));

    let mut projects = config
        .projects
        .iter()
        .map(project_summary)
        .collect::<Vec<_>>();
    projects.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(InspectReport {
        scope,
        skills_dir: config.skills_dir.to_string_lossy().into_owned(),
        active_project_id: config.active_project_id,
        projects,
        counts: CountSummary {
            total: skills.len(),
            managed,
            tool,
            packages: packages.len(),
        },
        skills,
        tools,
        packages,
        presets: config.presets.clone(),
    })
}

fn project_summary(project: &ProjectBinding) -> ProjectSummary {
    ProjectSummary {
        id: project.id.clone(),
        name: project.name.clone(),
        skills_dir: project.skills_dir.to_string_lossy().into_owned(),
        root_path: project
            .root_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
    }
}

fn project_list_report(config: &AppConfig) -> ProjectListReport {
    let mut projects = config
        .projects
        .iter()
        .map(project_summary)
        .collect::<Vec<_>>();
    projects.sort_by(|a, b| a.id.cmp(&b.id));
    ProjectListReport {
        active_project_id: config.active_project_id.clone(),
        projects,
    }
}

fn print_project_list(config: &AppConfig, json: bool) -> Result<(), String> {
    let report = project_list_report(config);
    if json {
        serde_json::to_string_pretty(&report)
            .map(|output| println!("{output}"))
            .map_err(|error| format!("Failed to serialize project list: {error}"))
    } else {
        println!(
            "Projects (active={}):",
            report.active_project_id.as_deref().unwrap_or("none")
        );
        if report.projects.is_empty() {
            println!("  (none)");
        } else {
            for project in report.projects {
                let active = if report.active_project_id.as_deref() == Some(project.id.as_str()) {
                    " active"
                } else {
                    ""
                };
                println!("  {}{}  {}", project.id, active, project.name);
                println!("    skills={}", project.skills_dir);
                if let Some(root_path) = project.root_path {
                    println!("    root={root_path}");
                }
            }
        }
        Ok(())
    }
}

fn print_project_preview(binding: &ProjectBinding, json: bool) -> Result<(), String> {
    if json {
        serde_json::to_string_pretty(binding)
            .map(|output| println!("{output}"))
            .map_err(|error| format!("Failed to serialize project preview: {error}"))
    } else {
        println!("Project preview: {} ({})", binding.name, binding.id);
        println!("  skills={}", binding.skills_dir.display());
        println!(
            "  root={}",
            binding
                .root_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string())
        );
        Ok(())
    }
}

fn load_persisted_config() -> Result<AppConfig, String> {
    let config_path = home_dir()
        .ok_or_else(|| "Could not resolve the current user's home directory".to_string())?
        .join(".skills-manager")
        .join("config.json");
    let config_content = fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {error}", config_path.display()))?;
    let mut config: AppConfig = serde_json::from_str(&config_content)
        .map_err(|error| format!("Failed to parse {}: {error}", config_path.display()))?;

    // Keep inspect read-only while matching the UI's effective config model:
    // existing installations get the built-in presets in memory even before a
    // mutating command persists the normal ConfigManager migration.
    for builtin in builtin_skill_activation_presets() {
        if !config.presets.iter().any(|preset| preset.id == builtin.id) {
            config.presets.push(builtin);
        }
    }

    Ok(config)
}

fn print_human(report: &InspectReport) {
    println!("Scope: {}", report.scope);
    println!("Global skills directory: {}", report.skills_dir);
    println!(
        "Counts: total={} managed={} direct-tool={} groups={}",
        report.counts.total, report.counts.managed, report.counts.tool, report.counts.packages
    );

    println!("\nManaged skills:");
    for skill in report
        .skills
        .iter()
        .filter(|skill| skill.scope != SkillScope::Tool)
    {
        println!(
            "  {}  [{}]  {}  {}",
            skill.instance_id,
            format_scope(skill),
            skill.name,
            skill.path.display()
        );
    }

    println!("\nDirect Tool skills:");
    for skill in report
        .skills
        .iter()
        .filter(|skill| skill.scope == SkillScope::Tool)
    {
        println!(
            "  {}  [{}]  {}  {}",
            skill.instance_id,
            skill.tool_id.as_deref().unwrap_or("unknown-tool"),
            skill.name,
            skill.path.display()
        );
    }

    println!("\nTool summary:");
    for tool in &report.tools {
        println!(
            "  {} ({}) detected={} enabled={} direct_skills={}",
            tool.id,
            tool.name.as_deref().unwrap_or("unknown"),
            tool.detected,
            tool.enabled,
            tool.direct_skill_count
        );
    }

    println!("\nSkill Groups:");
    if report.packages.is_empty() {
        println!("  (none)");
    } else {
        for package in &report.packages {
            println!(
                "  {}  members={} selected={}",
                package.package_id,
                package.installed_members.join(","),
                package.selected_members.join(",")
            );
        }
    }

    println!("\nPresets:");
    if report.presets.is_empty() {
        println!("  (none)");
    } else {
        for preset in &report.presets {
            let activations = preset
                .activations
                .iter()
                .map(|activation| format!("{}:{}", activation.tool_id, activation.skill_ids.len()))
                .collect::<Vec<_>>()
                .join(",");
            println!("  {}  activations={}", preset.id, activations);
        }
    }
}

fn print_provider_inventory(inventory: &SkillProviderInventory) {
    println!("Provider inventory (checked_at={}):", inventory.checked_at);
    for provider in &inventory.providers {
        let reachable = provider
            .reachable
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        println!(
            "  {} [{}] detected={} cli={} reachable={} skills={} enabled={} disabled={}",
            provider.provider_id,
            provider_kind_label(&provider.kind),
            provider.detected,
            provider.cli_available,
            reachable,
            provider.skill_count,
            provider.enabled_count,
            provider.disabled_count
        );
        if let Some(root_path) = &provider.root_path {
            println!("    root={}", root_path.display());
        }
        if let Some(warning) = &provider.warning {
            println!("    warning={warning}");
        }
    }

    println!(
        "Orca: cli={} available={} app_running={} reachable={} state={} topics={} topics_available={}",
        inventory.orca.cli_available,
        inventory.orca.available,
        inventory
            .orca
            .app_running
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        inventory
            .orca
            .runtime_reachable
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        inventory.orca.runtime_state.as_deref().unwrap_or("unknown"),
        inventory.orca.topics.len(),
        inventory.orca.topics_available,
    );
    for topic in &inventory.orca.topics {
        println!("  topic {}", topic.name);
    }
    if let Some(warning) = &inventory.orca.warning {
        println!("  warning={warning}");
    }
}

fn provider_kind_label(
    kind: &tmpskills_manager_temp_lib::models::SkillProviderKind,
) -> &'static str {
    match kind {
        tmpskills_manager_temp_lib::models::SkillProviderKind::Filesystem => "filesystem",
        tmpskills_manager_temp_lib::models::SkillProviderKind::ConfigFile => "config_file",
        tmpskills_manager_temp_lib::models::SkillProviderKind::Cli => "cli",
        tmpskills_manager_temp_lib::models::SkillProviderKind::Marketplace => "marketplace",
    }
}

fn binding_state_label(
    state: &tmpskills_manager_temp_lib::models::SkillBindingState,
) -> &'static str {
    match state {
        tmpskills_manager_temp_lib::models::SkillBindingState::Enabled => "enabled",
        tmpskills_manager_temp_lib::models::SkillBindingState::Disabled => "disabled",
        tmpskills_manager_temp_lib::models::SkillBindingState::Missing => "missing",
        tmpskills_manager_temp_lib::models::SkillBindingState::Conflict => "conflict",
        tmpskills_manager_temp_lib::models::SkillBindingState::Unavailable => "unavailable",
    }
}

fn binding_scope_label(binding: &SkillBinding) -> String {
    match &binding.scope {
        SkillScope::Global => "global".to_string(),
        SkillScope::Project => "project".to_string(),
        SkillScope::Tool => "tool".to_string(),
    }
}

fn print_skill_bindings(bindings: &[SkillBinding]) {
    println!("Skill bindings ({}):", bindings.len());
    if bindings.is_empty() {
        println!("  (none)");
        return;
    }
    for binding in bindings {
        println!(
            "  {}  provider={} scope={} state={}",
            binding.skill_instance_id,
            binding.provider_id,
            binding_scope_label(binding),
            binding_state_label(&binding.state),
        );
        if let Some(source_path) = &binding.source_path {
            println!("    source={}", source_path.display());
        }
        if let Some(target_path) = &binding.target_path {
            println!("    target={}", target_path.display());
        }
        if let Some(reason) = &binding.reason {
            println!("    reason={reason}");
        }
    }
}

fn format_scope(skill: &Skill) -> String {
    match &skill.scope {
        SkillScope::Global => "global".to_string(),
        SkillScope::Project => format!(
            "project:{}",
            skill.project_id.as_deref().unwrap_or("unknown-project")
        ),
        SkillScope::Tool => "tool".to_string(),
    }
}

fn print_operation_result(operation: &str, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "operation": operation,
            })
        );
    } else {
        println!("Completed: {operation}");
    }
}

fn print_operation_report(report: &SkillOperationReport, json: bool) -> Result<(), String> {
    if json {
        serde_json::to_string_pretty(report)
            .map(|output| println!("{output}"))
            .map_err(|error| format!("Failed to serialize operation report: {error}"))
    } else {
        println!(
            "Operation {}: requested={} attempted={} applied={} skipped={} failed={}",
            report.operation_id,
            report.requested_count,
            report.attempted_count,
            report.applied_count,
            report.skipped_count,
            report.failed_count,
        );
        if !report.impacts.is_empty() {
            println!(
                "  impacts={}",
                report
                    .impacts
                    .iter()
                    .map(|impact| impact.provider_id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        for failure in &report.failures {
            println!(
                "  failed skill={} provider={} reason={}",
                failure.skill_instance_id.as_deref().unwrap_or("-"),
                failure.provider_id.as_deref().unwrap_or("-"),
                failure.message,
            );
        }
        Ok(())
    }
}

fn print_operation_preview(preview: &SkillOperationPreview, json: bool) -> Result<(), String> {
    if json {
        serde_json::to_string_pretty(preview)
            .map(|output| println!("{output}"))
            .map_err(|error| format!("Failed to serialize operation preview: {error}"))
    } else {
        println!(
            "Preview: skill={} provider={} confirm={}",
            preview.skill_instance_id, preview.provider_id, preview.requires_confirmation
        );
        for impact in &preview.impacts {
            println!(
                "  impact={} root={}",
                impact.provider_id,
                impact
                    .root_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
        }
        if let Some(warning) = &preview.warning {
            println!("  warning={warning}");
        }
        Ok(())
    }
}

fn print_preset_result(
    operation: &str,
    preset: &SkillActivationPreset,
    json: bool,
) -> Result<(), String> {
    if json {
        serde_json::to_string_pretty(preset)
            .map(|output| println!("{output}"))
            .map_err(|error| format!("Failed to serialize {operation} result: {error}"))
    } else {
        let activation_count = preset.activations.len();
        let skill_count = preset
            .activations
            .iter()
            .map(|activation| activation.skill_ids.len())
            .sum::<usize>();
        println!(
            "Preset {operation}: {} ({}) activations={} skills={}",
            preset.id, preset.name, activation_count, skill_count
        );
        Ok(())
    }
}

fn print_batch_result(response: &BatchSetSkillToolsResponse, json: bool) -> Result<(), String> {
    if json {
        let output = serde_json::to_string_pretty(response)
            .map_err(|error| format!("Failed to serialize batch response: {error}"))?;
        println!("{output}");
    } else {
        println!(
            "Batch complete: applied={} skipped={} failed={} resolved={}",
            response.applied_count,
            response.skipped_count,
            response.failed_count,
            response.resolved_skill_count
        );
        if !response.report.impacts.is_empty() {
            println!(
                "  impacts={}",
                response
                    .report
                    .impacts
                    .iter()
                    .map(|impact| impact.provider_id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        for failure in &response.failures {
            println!(
                "  failed target={} tool={} skill={} reason={}",
                failure.target_id,
                failure.tool_id.as_deref().unwrap_or("-"),
                failure.skill_id.as_deref().unwrap_or("-"),
                failure.message
            );
        }
    }
    Ok(())
}

fn print_local_contract_result(summary: &SkillContractSummary, json: bool) -> Result<(), String> {
    if json {
        serde_json::to_string_pretty(summary)
            .map(|output| println!("{output}"))
            .map_err(|error| format!("Failed to serialize local contract: {error}"))
    } else {
        println!(
            "Local contract saved: status={:?} source={}",
            summary.status,
            summary
                .source
                .as_ref()
                .map(|source| format!("{source:?}"))
                .unwrap_or_else(|| "none".to_string())
        );
        if !summary.validation_errors.is_empty() {
            println!(
                "  validation_errors={}",
                summary.validation_errors.join("; ")
            );
        }
        Ok(())
    }
}

fn main() {
    let command = match parse_command() {
        Ok(command) => command,
        Err(error) => {
            eprintln!("Error: {error}");
            eprintln!();
            usage();
            process::exit(2);
        }
    };

    let result = match command {
        Command::Inspect(options) => {
            let json = options.json;
            match inspect(options) {
                Ok(report) if json => serde_json::to_string_pretty(&report)
                    .map(|output| println!("{output}"))
                    .map_err(|error| format!("Failed to serialize inspection report: {error}")),
                Ok(report) => {
                    print_human(&report);
                    Ok(())
                }
                Err(error) => Err(format!("Inspection failed: {error}")),
            }
        }
        Command::Projects { json } => {
            load_persisted_config().and_then(|config| print_project_list(&config, json))
        }
        Command::PreviewProject { path, name, json } => {
            WorkspaceService::preview_project(&path, name.as_deref())
                .and_then(|binding| print_project_preview(&binding, json))
        }
        Command::AddProject { path, name, json } => {
            WorkspaceService::register_project(&path, name.as_deref())
                .and_then(|config| print_project_list(&config, json))
        }
        Command::UseProject { project_id, json } => {
            WorkspaceService::set_active_project(Some(&project_id))
                .and_then(|config| print_project_list(&config, json))
        }
        Command::RemoveProject { project_id, json } => {
            WorkspaceService::remove_project(&project_id)
                .and_then(|config| print_project_list(&config, json))
        }
        Command::Providers { project_id, json } => {
            let config = load_persisted_config();
            let inventory = config.and_then(|config| {
                let skills = ScannerService::scan_skills_for_scope(&config, project_id.as_deref())?;
                Ok(ProviderInventoryService::list_with_skills(&config, &skills))
            });
            inventory.and_then(|inventory| {
                if json {
                    serde_json::to_string_pretty(&inventory)
                        .map(|output| println!("{output}"))
                        .map_err(|error| format!("Failed to serialize provider inventory: {error}"))
                } else {
                    print_provider_inventory(&inventory);
                    Ok(())
                }
            })
        }
        Command::Bindings {
            project_id,
            provider_id,
            skill_instance_id,
            json,
        } => {
            let config = load_persisted_config();
            let bindings = config.and_then(|config| {
                let skills = ScannerService::scan_skills_for_scope(&config, project_id.as_deref())?;
                Ok(ProviderInventoryService::list_bindings_with_skills(
                    &config,
                    &skills,
                    provider_id.as_deref(),
                    skill_instance_id.as_deref(),
                ))
            });
            bindings.and_then(|bindings| {
                if json {
                    serde_json::to_string_pretty(&bindings)
                        .map(|output| println!("{output}"))
                        .map_err(|error| format!("Failed to serialize skill bindings: {error}"))
                } else {
                    print_skill_bindings(&bindings);
                    Ok(())
                }
            })
        }
        Command::PreviewSkill {
            instance_id,
            tool_id,
            project_id,
            enabled,
            json,
        } => {
            let preview = load_persisted_config().and_then(|config| {
                let skills = ScannerService::scan_skills_for_scope(&config, project_id.as_deref())?;
                ProviderInventoryService::preview_binding_operation_with_skills(
                    &config,
                    &skills,
                    project_id.as_deref(),
                    &instance_id,
                    &tool_id,
                    enabled,
                )
            });
            preview.and_then(|preview| print_operation_preview(&preview, json))
        }
        Command::SetSkill {
            instance_id,
            tool_id,
            project_id,
            enabled,
            confirm_shared,
            json,
        } => {
            let config = load_persisted_config();
            let preview = config.and_then(|config| {
                let skills = ScannerService::scan_skills_for_scope(&config, project_id.as_deref())?;
                ProviderInventoryService::preview_binding_operation_with_skills(
                    &config,
                    &skills,
                    project_id.as_deref(),
                    &instance_id,
                    &tool_id,
                    enabled,
                )
            });
            preview.and_then(|preview| {
                if preview.requires_confirmation && !confirm_shared {
                    Err(format!(
                        "{}; re-run with --confirm-shared",
                        preview.warning.unwrap_or_else(|| {
                            "Shared provider impact requires confirmation".to_string()
                        })
                    ))
                } else {
                    SkillControlService::set_skill_enabled_for_scope(
                        project_id.as_deref(),
                        &instance_id,
                        &tool_id,
                        enabled,
                    )
                    .and_then(|report| print_operation_report(&report, json))
                }
            })
        }
        Command::CreateSkill {
            name,
            description,
            json,
        } => SkillControlService::create_skill(&name, description.as_deref()).and_then(|skill| {
            if json {
                serde_json::to_string_pretty(&skill)
                    .map(|output| println!("{output}"))
                    .map_err(|error| format!("Failed to serialize created skill: {error}"))
            } else {
                println!(
                    "Created skill: {} ({})",
                    skill.instance_id,
                    skill.path.display()
                );
                Ok(())
            }
        }),
        Command::DeleteSkill { instance_id, json } => {
            SkillControlService::delete_skill(&instance_id)
                .map(|_| print_operation_result("skill.delete", json))
        }
        Command::ImportSkills { paths, json } => SkillControlService::import_skills_to_hub(&paths)
            .map(|_| print_operation_result("skill.import", json)),
        Command::SetLocalSkillContract {
            instance_id,
            file,
            json,
        } => fs::read_to_string(&file)
            .map_err(|error| format!("Failed to read {file}: {error}"))
            .and_then(|contents| {
                serde_yaml::from_str::<SkillContract>(&contents)
                    .map_err(|error| format!("Failed to parse {file} as contract YAML: {error}"))
            })
            .and_then(|contract| {
                SkillControlService::save_local_skill_contract(SaveLocalSkillContractRequest {
                    skill_instance_id: instance_id,
                    contract,
                })
            })
            .and_then(|summary| print_local_contract_result(&summary, json)),
        Command::SetTool {
            tool_id,
            enabled,
            json,
        } => ToolControlService::set_enabled(&tool_id, enabled).map(|_| {
            print_operation_result(
                if enabled {
                    "tool.enable"
                } else {
                    "tool.disable"
                },
                json,
            )
        }),
        Command::BatchSet { request, json } => SkillControlService::batch_set_skill_tools(request)
            .and_then(|response| print_batch_result(&response, json)),
        Command::ApplyPreset {
            preset_id,
            project_id,
            tool_id,
            json,
        } => {
            let operation = if let Some(tool_id) = tool_id {
                SkillControlService::apply_preset_for_target(
                    &preset_id,
                    project_id.as_deref(),
                    &tool_id,
                )
                .and_then(|report| print_operation_report(&report, json))
            } else {
                SkillControlService::apply_preset_for_scope(&preset_id, project_id.as_deref())
                    .and_then(|report| print_operation_report(&report, json))
            };
            operation
        }
        Command::CreatePreset {
            name,
            description,
            copy_current_state,
            project_id,
            tool_id,
            json,
        } => SkillControlService::create_preset(
            &name,
            description.as_deref(),
            copy_current_state,
            project_id.as_deref(),
            tool_id.as_deref(),
        )
        .and_then(|preset| print_preset_result("created", &preset, json)),
        Command::DeletePreset { preset_id, json } => SkillControlService::delete_preset(&preset_id)
            .map(|_| print_operation_result("preset.delete", json)),
        Command::CapturePreset {
            preset_id,
            project_id,
            tool_id,
            json,
        } => SkillControlService::capture_preset(&preset_id, project_id.as_deref(), &tool_id)
            .and_then(|preset| print_preset_result("captured", &preset, json)),
        Command::SetPresetSkill {
            preset_id,
            project_id,
            tool_id,
            skill_id,
            enabled,
            json,
        } => SkillControlService::set_preset_skill(
            &preset_id,
            project_id.as_deref(),
            &tool_id,
            &skill_id,
            enabled,
        )
        .and_then(|preset| print_preset_result("updated", &preset, json)),
        Command::SetPresetAll {
            preset_id,
            project_id,
            tool_id,
            enabled,
            json,
        } => SkillControlService::set_preset_all(
            &preset_id,
            project_id.as_deref(),
            &tool_id,
            enabled,
        )
        .and_then(|preset| print_preset_result("updated", &preset, json)),
        Command::ClearPreset { json } => SkillControlService::clear_active_preset()
            .map(|_| print_operation_result("preset.clear", json)),
    };

    if let Err(error) = result {
        eprintln!("Operation failed: {error}");
        process::exit(1);
    }
}
