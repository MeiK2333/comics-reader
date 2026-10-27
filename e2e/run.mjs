import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const isWindows = process.platform === "win32";
const identifier = "com.meik.comics-reader.e2e";
// Allow a separately built E2E binary (for example when `pnpm tauri dev` is
// holding the default target/debug executable open).
const appPath = process.env.TAURI_E2E_APP
  ? path.resolve(root, process.env.TAURI_E2E_APP)
  : path.join(
      root,
      "src-tauri",
      "target",
      "debug",
      isWindows ? "comics-reader.exe" : "comics-reader",
    );
const driverPath = path.join(
  root,
  "e2e",
  ".drivers",
  isWindows ? "msedgedriver.exe" : "msedgedriver",
);
const dataRoot = isWindows
  ? process.env.APPDATA
  : (process.env.XDG_DATA_HOME ?? path.join(os.homedir(), ".local", "share"));
const appData = path.join(dataRoot, identifier);

// A 1x1 PNG is enough: the reader only needs a decodable file behind the
// asset protocol, not a realistic page.
const onePixelPng = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=",
  "base64",
);

/**
 * Seed the E2E app data directory. Downloads and favorites are real files the
 * production stores read, so the tests exercise the same load/parse paths as
 * a user's offline library.
 */
async function seed() {
  await rm(appData, { recursive: true, force: true });
  await mkdir(appData, { recursive: true });

  const chapterDir = path.join(
    appData,
    "downloads",
    "nhentai",
    "900001",
    "single",
  );
  await mkdir(chapterDir, { recursive: true });
  await writeFile(path.join(chapterDir, "00001.png"), onePixelPng);
  await writeFile(path.join(chapterDir, "00002.png"), onePixelPng);
  await writeFile(
    path.join(chapterDir, "manifest.json"),
    JSON.stringify(
      {
        version: 1,
        chapterId: null,
        chapterTitle: null,
        pages: [
          {
            url: "https://e2e.local/page-1.png",
            file: "00001.png",
            width: 1,
            height: 1,
          },
          {
            url: "https://e2e.local/page-2.png",
            file: "00002.png",
            width: 1,
            height: 1,
          },
        ],
      },
      null,
      2,
    ),
  );
  await writeFile(
    path.join(appData, "downloads.json"),
    JSON.stringify(
      {
        version: 1,
        entries: [
          {
            platform: "nhentai",
            id: "900001",
            title: "E2E Offline Gallery",
            thumb: null,
            status: "completed",
            chapters: [
              {
                id: null,
                title: null,
                pageCount: 2,
                completed: true,
              },
            ],
            downloadedPages: 2,
            totalBytes: onePixelPng.length * 2,
            addedAt: 1_700_000_000_000,
            updatedAt: 1_700_000_000_000,
          },
        ],
      },
      null,
      2,
    ),
  );
  await writeFile(
    path.join(appData, "favorites.json"),
    JSON.stringify(
      {
        version: 1,
        groups: [
          { id: "default", name: "", createdAt: 0 },
          { id: "g1", name: "E2E Group", createdAt: 1_700_000_000_000 },
        ],
        entries: [
          {
            platform: "nhentai",
            id: "e2e-fav-1",
            title: "E2E Favorite One",
            thumb: null,
            addedAt: 1_700_000_000_001,
            groupId: "default",
          },
          {
            platform: "nhentai",
            id: "e2e-fav-2",
            title: "E2E Favorite Two",
            thumb: null,
            addedAt: 1_700_000_000_002,
            groupId: "g1",
          },
        ],
      },
      null,
      2,
    ),
  );
}

function waitForPort(port, timeoutMs = 20_000) {
  const started = Date.now();

  return new Promise((resolve, reject) => {
    const attempt = () => {
      const socket = net.connect({ host: "127.0.0.1", port });
      socket.once("connect", () => {
        socket.destroy();
        resolve();
      });
      socket.once("error", () => {
        socket.destroy();
        if (Date.now() - started > timeoutMs) {
          reject(new Error(`timed out waiting for tauri-driver on port ${port}`));
          return;
        }
        setTimeout(attempt, 200);
      });
    };
    attempt();
  });
}

async function main() {
  if (!existsSync(appPath)) {
    throw new Error(`missing ${appPath}; run \`pnpm build:e2e\` first`);
  }
  if (!existsSync(driverPath)) {
    throw new Error(
      `missing ${driverPath}; download the Edge Driver matching Edge/WebView2 into e2e/.drivers`,
    );
  }

  await seed();

  const driverArgs = ["--port", "4444"];
  if (isWindows) {
    driverArgs.push("--native-driver", driverPath);
  }
  const driver = spawn("tauri-driver", driverArgs, {
    cwd: root,
    stdio: "inherit",
  });

  let exitCode = 1;
  try {
    await waitForPort(4444);

    const wdioPath = path.join(
      root,
      "node_modules",
      ".bin",
      isWindows ? "wdio.cmd" : "wdio",
    );
    const wdio = spawn(wdioPath, ["run", path.join(root, "e2e", "wdio.conf.js")], {
      cwd: root,
      env: { ...process.env, TAURI_E2E_APP: appPath },
      shell: isWindows,
      stdio: "inherit",
    });

    exitCode = await new Promise((resolve, reject) => {
      wdio.once("exit", (code) => resolve(code ?? 1));
      wdio.once("error", reject);
    });
  } finally {
    driver.kill();
    if (!process.env.E2E_KEEP) {
      await rm(appData, { recursive: true, force: true }).catch(() => undefined);
    }
  }

  process.exitCode = exitCode;
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
