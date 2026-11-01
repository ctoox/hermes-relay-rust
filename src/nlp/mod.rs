//! Hermes Relay - NLP 自然语言解析模块

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// NLP 规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub patterns: Vec<String>,
    pub command: String,
    pub description: String,
    pub extract_args: bool,
    pub require_approval: bool,
    pub special_action: String,
}

/// NLP 解析结果
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub command: String,
    pub description: String,
    pub require_approval: bool,
    pub special_action: String,
    pub args: Vec<String>,
}

/// NLP 解析器
pub struct NlpParser {
    rules: Vec<CompiledRule>,
}

struct CompiledRule {
    patterns: Vec<Regex>,
    command: String,
    description: String,
    extract_args: bool,
    require_approval: bool,
    special_action: String,
}

impl NlpParser {
    pub fn new() -> Self {
        let mut parser = Self {
            rules: Vec::new(),
        };
        parser.load_default_rules();
        parser
    }
    
    fn load_default_rules(&mut self) {
        // 查看目录
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:查看|显示|列出|看看)(?:当前)?(?:目录|文件夹|文件)(?:内容)?".to_string(),
                r"(?i)(?:list|show|view)(?:\s+current)?(?:\s+directory|\s+folder|\s+files)?".to_string(),
                r"(?i)^dir$".to_string(),
                r"(?i)^ls$".to_string(),
            ],
            command: "dir".to_string(),
            description: "查看当前目录内容".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 查看指定目录
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:查看|显示|列出)(?:目录|文件夹)\s*(.+)".to_string(),
                r"(?i)(?:list|show|view)(?:\s+directory|\s+folder)\s+(.+)".to_string(),
            ],
            command: "dir {0}".to_string(),
            description: "查看指定目录内容".to_string(),
            extract_args: true,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 切换目录
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:切换|进入|打开)(?:到)?(?:目录|文件夹)\s*(.+)".to_string(),
                r"(?i)(?:cd|change\s+to|goto)\s+(.+)".to_string(),
            ],
            command: "cd /d {0}".to_string(),
            description: "切换目录".to_string(),
            extract_args: true,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 创建目录
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:创建|新建|建立)(?:目录|文件夹)\s*(.+)".to_string(),
                r"(?i)(?:create|make|new)(?:\s+directory|\s+folder)\s+(.+)".to_string(),
                r"(?i)mkdir\s+(.+)".to_string(),
            ],
            command: "mkdir {0}".to_string(),
            description: "创建目录".to_string(),
            extract_args: true,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 查看文件内容
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:查看|显示|打开|读取)(?:文件)?\s*(.+?)(?:\s*的)?(?:内容)?".to_string(),
                r"(?i)(?:cat|type|view|read|show)\s+(.+)".to_string(),
            ],
            command: "type {0}".to_string(),
            description: "查看文件内容".to_string(),
            extract_args: true,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 查看IP地址
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:查看|显示|看看)(?:本机)?(?:IP|ip|网络|网卡)(?:地址|信息)?".to_string(),
                r"(?i)(?:show|view|check|display)(?:\s+my)?(?:\s+ip|\s+network|\s+nic)?".to_string(),
                r"(?i)^ipconfig$".to_string(),
            ],
            command: "ipconfig /all".to_string(),
            description: "查看IP配置".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 查看系统信息
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:查看|显示|看看)(?:系统|电脑|计算机)(?:信息|配置|状态)".to_string(),
                r"(?i)(?:show|view|check)(?:\s+system)?(?:\s+info|\s+config|\s+status)?".to_string(),
                r"(?i)^systeminfo$".to_string(),
            ],
            command: "systeminfo".to_string(),
            description: "查看系统信息".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 查看进程
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:查看|显示|列出|看看)(?:当前)?(?:进程|程序|任务)".to_string(),
                r"(?i)(?:list|show|view)(?:\s+current)?(?:\s+process|\s+program|\s+task)?".to_string(),
                r"(?i)^tasklist$".to_string(),
            ],
            command: "tasklist /V".to_string(),
            description: "查看进程列表".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 查找指定进程
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:查看|查找|搜索)(?:进程)?\s*(.+?)(?:\s*进程)?".to_string(),
                r"(?i)(?:find|search|look\s+for)(?:\s+process)?\s+(.+)".to_string(),
            ],
            command: r#"tasklist /FI "IMAGENAME eq {0}*""#.to_string(),
            description: "查找指定进程".to_string(),
            extract_args: true,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 关闭进程 (需要审批)
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:关闭|结束|杀死|停止|杀掉)(?:进程)?\s*(.+?)(?:\s*进程)?".to_string(),
                r"(?i)(?:kill|stop|close|terminate|end)(?:\s+process)?\s+(.+)".to_string(),
                r"(?i)taskkill\s+(.+)".to_string(),
            ],
            command: "taskkill /IM {0} /F".to_string(),
            description: "关闭进程".to_string(),
            extract_args: true,
            require_approval: true,
            special_action: String::new(),
        });
        
        // Ping测试
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:ping|测试|检查)(?:到)?\s*(.+?)(?:\s*的)?(?:\s*连接)?".to_string(),
                r"(?i)ping\s+(.+)".to_string(),
            ],
            command: "ping -n 4 {0}".to_string(),
            description: "Ping测试".to_string(),
            extract_args: true,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 查看端口
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:查看|显示|检查)(?:当前)?(?:端口|连接|网络状态)".to_string(),
                r"(?i)(?:show|view|check)(?:\s+current)?(?:\s+port|\s+connection|\s+network)?".to_string(),
                r"(?i)^netstat$".to_string(),
            ],
            command: "netstat -ano".to_string(),
            description: "查看网络连接".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 打开记事本
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:打开|启动|运行)记事本".to_string(),
                r"(?i)(?:open|start|run)\s+notepad".to_string(),
                r"(?i)^notepad$".to_string(),
            ],
            command: "start notepad".to_string(),
            description: "打开记事本".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 打开计算器
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:打开|启动|运行)计算器".to_string(),
                r"(?i)(?:open|start|run)\s+calc(?:ulator)?".to_string(),
                r"(?i)^calc$".to_string(),
            ],
            command: "start calc".to_string(),
            description: "打开计算器".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 打开命令提示符
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:打开|启动|运行)(?:命令行|cmd|终端|命令提示符)".to_string(),
                r"(?i)(?:open|start|run)\s+(?:cmd|command|terminal)".to_string(),
                r"(?i)^cmd$".to_string(),
            ],
            command: "start cmd".to_string(),
            description: "打开命令提示符".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 打开资源管理器
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:打开|启动|运行)(?:资源管理器|文件管理器|我的电脑|此电脑)".to_string(),
                r"(?i)(?:open|start|run)\s+(?:explorer|file\s+manager)".to_string(),
                r"(?i)^explorer$".to_string(),
            ],
            command: "start explorer".to_string(),
            description: "打开资源管理器".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 打开任务管理器
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:打开|启动|运行)(?:任务管理器)".to_string(),
                r"(?i)(?:open|start|run)\s+task\s*manager".to_string(),
                r"(?i)^taskmgr$".to_string(),
            ],
            command: "start taskmgr".to_string(),
            description: "打开任务管理器".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 截图
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:截图|截屏|屏幕截图|抓屏)".to_string(),
                r"(?i)(?:screenshot|capture|screen\s*shot)".to_string(),
            ],
            command: "screenshot".to_string(),
            description: "截图".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: "screenshot".to_string(),
        });
        
        // 查看时间
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:查看|显示|看看)(?:当前)?(?:时间|日期)".to_string(),
                r"(?i)(?:show|what)\s+(?:time|date)".to_string(),
                r"(?i)^time$".to_string(),
                r"(?i)^date$".to_string(),
            ],
            command: "echo %date% %time%".to_string(),
            description: "查看当前时间".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 清屏
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:清屏|清除屏幕|清除)".to_string(),
                r"(?i)(?:clear|cls|clean)".to_string(),
                r"(?i)^cls$".to_string(),
            ],
            command: "cls".to_string(),
            description: "清屏".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: String::new(),
        });
        
        // 帮助
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:帮助|帮忙|怎么用|使用方法|命令)".to_string(),
                r"(?i)(?:help|usage|how\s+to|command)".to_string(),
                r"(?i)^help$".to_string(),
            ],
            command: "help".to_string(),
            description: "显示帮助".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: "help".to_string(),
        });
        
        // 设备列表
        self.add_rule(Rule {
            patterns: vec![
                r"(?i)(?:设备列表|设备|查看设备|显示设备)".to_string(),
                r"(?i)(?:device|device\s+list|show\s+device)".to_string(),
            ],
            command: "device_list".to_string(),
            description: "查看设备列表".to_string(),
            extract_args: false,
            require_approval: false,
            special_action: "device_list".to_string(),
        });
    }
    
    pub fn add_rule(&mut self, rule: Rule) {
        let compiled_patterns: Vec<Regex> = rule.patterns.iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();
        
        self.rules.push(CompiledRule {
            patterns: compiled_patterns,
            command: rule.command,
            description: rule.description,
            extract_args: rule.extract_args,
            require_approval: rule.require_approval,
            special_action: rule.special_action,
        });
    }
    
    pub fn parse(&self, text: &str) -> Option<ParseResult> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        
        for rule in &self.rules {
            for pattern in &rule.patterns {
                if let Some(captures) = pattern.captures(text) {
                    let mut command = rule.command.clone();
                    let mut args = Vec::new();
                    
                    if rule.extract_args {
                        for i in 1..captures.len() {
                            if let Some(m) = captures.get(i) {
                                args.push(m.as_str().to_string());
                                command = command.replace(&format!("{{{}}}", i - 1), m.as_str());
                            }
                        }
                    }
                    
                    debug!("[NLP] Matched rule: {} -> {}", text, command);
                    
                    return Some(ParseResult {
                        command,
                        description: rule.description.clone(),
                        require_approval: rule.require_approval,
                        special_action: rule.special_action.clone(),
                        args,
                    });
                }
            }
        }
        
        // 检查是否直接是命令
        if looks_like_command(text) {
            debug!("[NLP] Direct command: {}", text);
            return Some(ParseResult {
                command: text.to_string(),
                description: "直接命令".to_string(),
                require_approval: false,
                special_action: String::new(),
                args: Vec::new(),
            });
        }
        
        debug!("[NLP] No match for: {}", text);
        None
    }
}

fn looks_like_command(text: &str) -> bool {
    let cmd_prefixes = [
        "dir", "ls", "cd", "mkdir", "copy", "move", "del", "rm",
        "ipconfig", "ping", "netstat", "tasklist", "taskkill",
        "systeminfo", "whoami", "hostname", "ver", "type",
        "echo", "set", "path", "exit", "shutdown", "restart",
        "net", "sc", "reg", "powershell", "cmd", "cls", "clear",
    ];
    
    let first_word = text.split_whitespace().next().unwrap_or("").to_lowercase();
    cmd_prefixes.contains(&first_word.as_str())
}
