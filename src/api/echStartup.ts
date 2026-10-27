let echStartup: Promise<void> | null = null;

/**
 * Register an in-flight ECH bootstrap. Business commands await this gate so a
 * deep link cannot issue its first request through the ordinary client.
 */
export function beginEchStartup(task: Promise<void>): void {
  const tracked = task
    .catch(() => undefined)
    .finally(() => {
      if (echStartup === tracked) {
        echStartup = null;
      }
    });
  echStartup = tracked;
}

export async function waitForEchStartup(): Promise<void> {
  // Loop in case a newer toggle replaced the promise while we were waiting.
  while (echStartup) {
    await echStartup;
  }
}
