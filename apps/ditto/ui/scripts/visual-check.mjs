import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import puppeteer from "puppeteer-core";

const browserPath =
  process.env.DITTO_BROWSER_PATH ??
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const url = process.env.DITTO_VISUAL_URL ?? "http://127.0.0.1:1420/";
const root = path.resolve(import.meta.dirname, "../../../..");
const outputDir = path.join(root, "docs", "rewrite", "screenshots", "T5");
const userDataDir = await mkdtemp(path.join(tmpdir(), "ditto-t5-edge-"));

await mkdir(outputDir, { recursive: true });

const browser = await puppeteer.launch({
  executablePath: browserPath,
  headless: "new",
  userDataDir,
  args: [
    "--no-first-run",
    "--no-sandbox",
    "--disable-extensions",
    "--disable-background-networking",
  ],
  defaultViewport: { width: 900, height: 640, deviceScaleFactor: 1 },
});

const page = await browser.newPage();
const consoleMessages = [];
const pageErrors = [];
const checks = [];

page.on("console", (message) => {
  if (message.type() === "error" || message.type() === "warning") {
    consoleMessages.push({ type: message.type(), text: message.text() });
  }
});
page.on("pageerror", (error) => pageErrors.push(error.message));

async function wait(ms = 160) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

async function click(selector) {
  await page.waitForSelector(selector, { visible: true, timeout: 5000 });
  await page.click(selector);
  await wait();
}

async function selectTheme(optionIndex) {
  await click("[data-page=appearance]");
  await click(".select-control");
  await page.waitForSelector('[role="option"]', { visible: true, timeout: 5000 });
  const options = await page.$$('[role="option"]');
  if (!options[optionIndex]) {
    throw new Error(`Theme option ${optionIndex} was not found`);
  }
  await options[optionIndex].click();
  await wait(260);
}

async function screenshot(name) {
  await page.screenshot({
    path: path.join(outputDir, name),
    type: "png",
    captureBeyondViewport: false,
  });
}

async function inspectLayout(name) {
  const metrics = await page.evaluate(() => {
    const containers = [
      ".settings-app",
      ".app-header",
      ".app-body",
      ".nav-pane",
      ".main-pane",
      ".page-scroll",
      ...Array.from(document.querySelectorAll(".settings-card")),
      ...Array.from(document.querySelectorAll(".buffer-slot")),
      ...Array.from(document.querySelectorAll(".history-preview")),
    ];
    const viewport = { width: window.innerWidth, height: window.innerHeight };
    const outside = containers
      .filter((element) => element instanceof HTMLElement)
      .map((element) => ({
        selector:
          element.className && typeof element.className === "string"
            ? `${element.tagName.toLowerCase()}.${element.className.split(" ").filter(Boolean).join(".")}`
            : element.tagName.toLowerCase(),
        rect: element.getBoundingClientRect().toJSON(),
      }))
      .filter(
        ({ rect }) =>
          rect.left < -1 || rect.right > viewport.width + 1,
      );
    const unlabeledButtons = Array.from(document.querySelectorAll("button"))
      .filter((button) => {
        const text = button.textContent?.trim() ?? "";
        const label = button.getAttribute("aria-label") ?? button.getAttribute("title") ?? "";
        return !text && !label;
      })
      .map((button) => button.outerHTML.slice(0, 180));
    return {
      viewport,
      documentScrollWidth: document.documentElement.scrollWidth,
      bodyScrollWidth: document.body.scrollWidth,
      outside,
      unlabeledButtons,
      language: document.documentElement.lang,
      theme: document.documentElement.dataset.theme,
    };
  });
  checks.push({ name, ...metrics });
  if (
    metrics.documentScrollWidth > metrics.viewport.width + 1 ||
    metrics.bodyScrollWidth > metrics.viewport.width + 1 ||
    metrics.outside.length > 0 ||
    metrics.unlabeledButtons.length > 0
  ) {
    throw new Error(`Layout check failed for ${name}: ${JSON.stringify(metrics)}`);
  }
}

async function clickHistory() {
  await click("[data-page=data]");
  await click("[data-tab=history]");
  await page.waitForSelector(".history-table-wrap tbody tr", { visible: true, timeout: 8000 });
  await click(".history-table-wrap tbody tr");
  await wait(260);
}

async function clickButtonByText(text) {
  const clicked = await page.evaluate((label) => {
    const buttons = Array.from(document.querySelectorAll("button"));
    const target = buttons.find((button) => button.textContent?.trim() === label);
    if (!target) {
      return false;
    }
    target.click();
    return true;
  }, text);
  if (!clicked) {
    throw new Error(`Button "${text}" was not found`);
  }
  await wait(220);
}

try {
  await page.goto(url, { waitUntil: "networkidle0", timeout: 30000 });
  await page.waitForSelector(".settings-app", { visible: true, timeout: 10000 });

  await selectTheme(1);
  await click("[data-page=general]");
  await inspectLayout("light-en-general");
  await screenshot("light-en-general-100.png");

  for (const pageName of [
    "appearance",
    "hotkeys",
    "clipboard",
    "groups",
    "copyBuffers",
    "data",
    "languages",
    "about",
    "general",
  ]) {
    await click(`[data-page=${pageName}]`);
    await inspectLayout(`light-en-${pageName}-navigation`);
    const localeViolations = await page.evaluate(() =>
      Array.from(document.querySelectorAll(".page-message"))
        .map((element) => element.textContent?.trim() ?? "")
        .filter((text) => /missing locale|缺少语言键|fell back to English|回退为英文/i.test(text)),
    );
    if (localeViolations.length) {
      throw new Error(`Visible locale warning on ${pageName}: ${localeViolations.join(" | ")}`);
    }
  }

  await clickHistory();
  await inspectLayout("light-en-history");
  await screenshot("light-en-history-100.png");

  await selectTheme(2);
  await inspectLayout("dark-en-appearance");
  await screenshot("dark-en-appearance-100.png");
  await clickHistory();
  await screenshot("dark-en-history-100.png");

  await click("[data-page=languages]");
  await click("[data-locale=zh-CN]");
  await wait(320);
  await selectTheme(1);
  await click("[data-page=data]");
  await inspectLayout("light-zh-data");
  await screenshot("light-zh-data-100.png");

  await selectTheme(2);
  await click("[data-page=general]");
  await inspectLayout("dark-zh-general");
  await screenshot("dark-zh-general-100.png");

  await page.setViewport({ width: 900, height: 640, deviceScaleFactor: 1.5 });
  await inspectLayout("dark-zh-general-150");
  await screenshot("dark-zh-general-150.png");

  await page.setViewport({ width: 900, height: 640, deviceScaleFactor: 2 });
  await inspectLayout("dark-zh-general-200");
  await screenshot("dark-zh-general-200.png");

  await page.setViewport({ width: 720, height: 480, deviceScaleFactor: 1 });
  await inspectLayout("dark-zh-general-min-window");
  await screenshot("dark-zh-general-720x480.png");

  await page.setViewport({ width: 900, height: 640, deviceScaleFactor: 1 });
  await selectTheme(0);
  await click("[data-page=languages]");
  await click("[data-locale=en-US]");
  await wait(280);
  await page.emulateMediaFeatures([{ name: "forced-colors", value: "active" }]);
  await selectTheme(3);
  await inspectLayout("high-contrast-en-general");
  await screenshot("high-contrast-en-general-100.png");

  await click("[data-page=general]");
  const switches = await page.$$('[role="switch"]');
  if (!switches[0]) {
    throw new Error("General switch was not found");
  }
  await switches[0].click();
  await wait(120);
  await clickButtonByText("Apply");
  await page.waitForFunction(
    () => document.querySelector(".revision-label")?.textContent?.includes("2"),
    { timeout: 5000 },
  );
  await page.waitForFunction(
    () => document.body.textContent?.includes("Settings applied."),
    { timeout: 5000 },
  );
  checks.push({ name: "settings-apply-toast", status: "passed" });
  await wait(4100);

  await click("[data-page=hotkeys]");
  await click('button[aria-label="Record shortcut"]');
  await page.keyboard.down("Control");
  await page.keyboard.down("Shift");
  await page.keyboard.press("V");
  await page.keyboard.up("Shift");
  await page.keyboard.up("Control");
  await wait(260);
  const conflictVisible = await page.evaluate(() =>
    Array.from(document.querySelectorAll(".inline-message")).some((element) =>
      element.textContent?.includes("Shortcut conflict"),
    ),
  );
  if (!conflictVisible) {
    throw new Error("Hotkey conflict warning was not shown");
  }
  checks.push({ name: "hotkey-conflict-warning", status: "passed" });
  await clickButtonByText("Discard");

  await click("[data-page=data]");
  await clickButtonByText("Back up now");
  await page.waitForSelector('[role="dialog"]', { visible: true, timeout: 5000 });
  await page.keyboard.press("Escape");
  await page.waitForFunction(() => !document.querySelector('[role="dialog"]'), {
    timeout: 5000,
  });
  checks.push({ name: "destructive-confirmation-escape", status: "passed" });

  const cdp = await page.createCDPSession();
  await cdp.send("Accessibility.enable");
  const accessibility = await cdp.send("Accessibility.getFullAXTree");
  const unnamedInteractiveRoles = new Set(["button", "combobox", "textbox", "tab"]);
  const unnamedInteractive = accessibility.nodes
    .filter((node) => unnamedInteractiveRoles.has(node.role?.value))
    .filter((node) => !node.name?.value)
    .map((node) => ({ role: node.role?.value, name: node.name?.value ?? "" }));
  checks.push({ name: "accessibility-tree", unnamedInteractive });
  if (unnamedInteractive.length > 0) {
    throw new Error(`Unnamed interactive accessibility nodes: ${JSON.stringify(unnamedInteractive)}`);
  }

  const report = {
    url,
    browser: "Google Chrome",
    executablePath: browserPath,
    generatedAt: new Date().toISOString(),
    screenshots: [
      "light-en-general-100.png",
      "light-en-history-100.png",
      "dark-en-appearance-100.png",
      "dark-en-history-100.png",
      "light-zh-data-100.png",
      "dark-zh-general-100.png",
      "dark-zh-general-150.png",
      "dark-zh-general-200.png",
      "dark-zh-general-720x480.png",
      "high-contrast-en-general-100.png",
    ],
    consoleMessages,
    pageErrors,
    checks,
  };
  await writeFile(
    path.join(outputDir, "visual-check.json"),
    `${JSON.stringify(report, null, 2)}\n`,
    "utf8",
  );
  if (consoleMessages.length || pageErrors.length) {
    throw new Error(
      `Browser console was not clean: ${JSON.stringify({ consoleMessages, pageErrors })}`,
    );
  }
  console.log(`Visual checks and ${report.screenshots.length} screenshots passed.`);
} finally {
  await browser.close();
  await rm(userDataDir, { recursive: true, force: true });
}
