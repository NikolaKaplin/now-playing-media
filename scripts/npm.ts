/** npm is a .cmd script on Windows, which needs cmd.exe to start. */
export function npmCommand(...args: string[]): string[] {
  return process.platform === "win32"
    ? ["cmd.exe", "/d", "/s", "/c", ["npm", ...args].join(" ")]
    : ["npm", ...args];
}
