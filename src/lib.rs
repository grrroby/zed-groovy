use std::fs;

use zed_extension_api::{
    self as zed, CodeLabel, CodeLabelSpan, DownloadedFileType, Extension, GithubReleaseOptions,
    LanguageServerId, LanguageServerInstallationStatus, Os, Worktree, current_platform,
    download_file, latest_github_release,
    lsp::{Completion, CompletionKind},
    register_extension, set_language_server_installation_status,
};

const LANGUAGE_SERVER_REPOSITORY: &str = "grrroby/groovy-language-server";
const LANGUAGE_SERVER_JAR: &str = "groovy-language-server-all.jar";

struct GroovyExtension {
    cached_jar_path: Option<String>,
}

impl GroovyExtension {
    fn language_server_jar_path(
        &mut self,
        language_server_id: &LanguageServerId,
    ) -> zed::Result<String> {
        if let Some(path) = &self.cached_jar_path {
            if fs::metadata(path).is_ok_and(|stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = latest_github_release(
            LANGUAGE_SERVER_REPOSITORY,
            GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;
        let (platform, _arch) = current_platform();
        let asset_name = format!(
            "groovy-language-server-{os}",
            os = match platform {
                Os::Mac => "macOS",
                Os::Linux => "Linux",
                Os::Windows => "Windows",
            },
        );
        let asset_file = format!("{asset_name}.zip");
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_file)
            .ok_or_else(|| format!("no asset found matching {asset_file:?}"))?;
        let version_dir = format!("groovy-language-server-{}", release.version);
        let jar_path = format!("{version_dir}/{asset_name}/{LANGUAGE_SERVER_JAR}");

        if !fs::metadata(&jar_path).is_ok_and(|stat| stat.is_file()) {
            set_language_server_installation_status(
                language_server_id,
                &LanguageServerInstallationStatus::Downloading,
            );
            download_file(&asset.download_url, &version_dir, DownloadedFileType::Zip)
                .map_err(|e| format!("failed to download file: {e}"))?;

            if !fs::metadata(&jar_path).is_ok_and(|stat| stat.is_file()) {
                return Err(format!(
                    "downloaded language server is missing {jar_path:?}"
                ));
            }

            let entries =
                fs::read_dir(".").map_err(|e| format!("failed to list working directory {e}"))?;

            for entry in entries {
                let entry = entry.map_err(|e| format!("failed to load directory entry {e}"))?;

                if entry.file_name().to_str() != Some(&version_dir) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
        }

        self.cached_jar_path = Some(jar_path.clone());

        Ok(jar_path)
    }
}

impl Extension for GroovyExtension {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            cached_jar_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        _worktree: &Worktree,
    ) -> zed::Result<zed::Command> {
        Ok(zed::Command {
            command: "java".into(),
            args: vec![
                "-jar".into(),
                self.language_server_jar_path(language_server_id)?,
            ],
            env: Vec::new(),
        })
    }

    fn label_for_completion(
        &self,
        _language_server_id: &LanguageServerId,
        completion: Completion,
    ) -> Option<CodeLabel> {
        match completion.kind? {
            CompletionKind::Class | CompletionKind::Enum | CompletionKind::Interface => {
                Some(CodeLabel {
                    code: format!("{} variable", completion.label),
                    spans: vec![
                        CodeLabelSpan::code_range(0..completion.label.len()),
                        CodeLabelSpan::literal(format!(" (import {})", completion.detail?), None),
                    ],
                    filter_range: (0..completion.label.len()).into(),
                })
            }
            CompletionKind::Method => {
                let code = format!("{}()", completion.label);

                Some(CodeLabel {
                    spans: vec![CodeLabelSpan::code_range(0..code.len())],
                    code,
                    filter_range: (0..completion.label.len()).into(),
                })
            }
            CompletionKind::Variable => {
                let def = "def ";
                let code = format!("{def}{}", completion.label);

                Some(CodeLabel {
                    spans: vec![CodeLabelSpan::code_range(def.len()..code.len())],
                    code,
                    filter_range: (0..completion.label.len()).into(),
                })
            }
            _ => None,
        }
    }
}

register_extension!(GroovyExtension);
