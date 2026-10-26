export const config = {
  runner: "local",
  hostname: "127.0.0.1",
  port: 4444,
  path: "/",
  specs: ["./specs/**/*.e2e.js"],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      browserName: "wry",
      "tauri:options": {
        application: process.env.TAURI_E2E_APP,
      },
    },
  ],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 90_000,
  },
  reporters: ["spec"],
  logLevel: "warn",
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 1,
};
