export class Logger {
  info(...args: unknown[]) {
    const message = [...generatePrefix("INFO", "#3ABFF8"), ...args];
    console.log(...message)
  }
  warn(...args: unknown[]) {
    const message = [...generatePrefix("WARN", "#FBBD23"), ...args];
    console.log(...message)
  }
  error(...args: unknown[]) {
    const message = [...generatePrefix("ERROR", "#F87272"), ...args];
    console.log(...message)
  }
  debug(...args: unknown[]) {
    const message = [...generatePrefix("DEBUG", "#D926A9"), ...args];
    console.log(...message)
  }
}

export default new Logger()

function generatePrefix(namespace: string, color: string): string[] {
  return [`%c[${namespace}]:%c`, `color:${color}; font-weight:bold`, '']
}