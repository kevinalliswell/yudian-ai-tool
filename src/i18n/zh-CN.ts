export const zhCN = {
  appName: "宇电 AI 温控工具",
  subtitle: "Tauri v2 + React/TypeScript + Rust",
  sections: {
    connection: "连接管理",
    monitor: "实时监控",
    parameters: "参数设置",
    curves: "温控曲线",
  },
  runConfirmation: {
    title: "确认运行当前曲线？",
    summary: (segmentCount: number, totalMinutes: number) =>
      `${segmentCount} 段，${totalMinutes} 分钟（${(totalMinutes / 60).toFixed(1)} 小时）`,
  },
} as const;
