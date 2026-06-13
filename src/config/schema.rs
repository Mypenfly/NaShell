use std::collections::HashMap;

/// 完整配置
#[derive(Debug, Clone, Default)]
pub struct NashellConfig {
    /// 启动时执行的命令或文件
    pub opening: OpeningConfig,
    /// 提示符配置
    pub prompts: PromptsConfig,
    /// 用户配置的 NaCommand 注册表
    pub na_commands: HashMap<String, ExternalCmdConfig>,
    /// Alias 映射表
    pub aliases: HashMap<String, String>,
    /// Shell 配置
    pub shell: ShellConfig,
    /// 安全配置
    pub safety: SafetyConfig,
    /// 插件配置
    pub plugins: PluginsConfig,
}

/// 启动配置
#[derive(Debug, Clone, Default)]
pub struct OpeningConfig {
    /// 启动时执行的命令
    pub exec: Option<String>,
    /// 启动时执行的文件
    pub file: Option<String>,
}

/// 提示符配置
#[derive(Debug, Clone)]
pub struct PromptsConfig {
    /// 输入提示符前景色（绿色）
    pub input_prompt_fg: String,
    /// 输入提示符格式
    pub input_prompt_format: String,
    /// 多行输入续行提示符格式
    pub input_continue_format: String,
    /// 输出提示符格式
    pub output_prompt_format: String,
    /// 输出提示符前景色（灰色）
    pub output_prompt_fg: String,
    /// Bash 命令输出提示符前景色（亮黄色）
    pub bash_output_prompt_fg: String,
    /// Shell 类型标识前景色
    pub shell_type_fg: String,
}

impl Default for PromptsConfig {
    fn default() -> Self {
        PromptsConfig {
            input_prompt_fg: "green".to_string(),
            input_prompt_format: "{path} |> ".to_string(),
            input_continue_format: ">> ".to_string(),
            output_prompt_format: "@System #>>".to_string(),
            output_prompt_fg: "grey".to_string(),
            bash_output_prompt_fg: "bright_yellow".to_string(),
            shell_type_fg: "cyan".to_string(),
        }
    }
}

/// 外部 NaCommand 配置
#[derive(Debug, Clone)]
pub struct ExternalCmdConfig {
    /// 执行程序路径
    pub exec: String,
    /// 是否接受 long_argument
    pub long_argument: bool,
    /// 可选执行脚本
    pub exec_script: Option<String>,
}

/// Shell 配置
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// Shell 命令超时秒数
    pub timeout_secs: u64,
}

impl Default for ShellConfig {
    fn default() -> Self {
        ShellConfig {
            timeout_secs: 120,
        }
    }
}

/// 安全配置
#[derive(Debug, Clone, Default)]
pub struct SafetyConfig {
    /// 拒绝执行的命令模式列表
    pub deny_patterns: Vec<String>,
}

/// 插件配置
#[derive(Debug, Clone)]
pub struct PluginsConfig {
    /// 插件目录
    pub dir: String,
    /// 最大递归深度
    pub max_recursion_depth: u32,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        PluginsConfig {
            dir: ".config/nashell/plugins".to_string(),
            max_recursion_depth: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nashell_config_has_default() {
        let config = NashellConfig::default();
        assert_eq!(config.prompts.input_prompt_fg, "green");
        assert!(config.na_commands.is_empty());
        assert!(config.aliases.is_empty());
        assert_eq!(config.shell.timeout_secs, 120);
        assert!(config.safety.deny_patterns.is_empty());
        assert_eq!(config.plugins.max_recursion_depth, 3);
    }

    #[test]
    fn test_default_prompt_config() {
        let prompts = PromptsConfig::default();
        assert_eq!(prompts.input_prompt_fg, "green");
        assert_eq!(prompts.input_prompt_format, "{path} |> ");
        assert_eq!(prompts.input_continue_format, ">> ");
        assert_eq!(prompts.output_prompt_fg, "grey");
        assert_eq!(prompts.bash_output_prompt_fg, "bright_yellow");
        assert_eq!(prompts.shell_type_fg, "cyan");
    }

    #[test]
    fn test_default_opening_config() {
        let opening = OpeningConfig::default();
        assert!(opening.exec.is_none());
        assert!(opening.file.is_none());
    }

    #[test]
    fn test_default_shell_config() {
        let shell = ShellConfig::default();
        assert_eq!(shell.timeout_secs, 120);
    }

    #[test]
    fn test_default_plugins_config() {
        let plugins = PluginsConfig::default();
        assert_eq!(plugins.dir, ".config/nashell/plugins");
        assert_eq!(plugins.max_recursion_depth, 3);
    }
}
