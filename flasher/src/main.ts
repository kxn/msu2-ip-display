import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type UiDeviceStatus = {
  kind: string;
  title: string;
  device_name?: string;
  port_name?: string;
  vid_pid?: string;
  serial?: string;
  button_enabled: boolean;
};

type FlashProgress = {
  percent: number;
  display_message: string;
};

type UiError = {
  message: string;
  detail: string;
};

const statusPill = document.querySelector<HTMLSpanElement>("#statusPill")!;
const deviceName = document.querySelector<HTMLDivElement>("#deviceName")!;
const devicePort = document.querySelector<HTMLDivElement>("#devicePort")!;
const rescanButton = document.querySelector<HTMLButtonElement>("#rescanButton")!;
const flashButton = document.querySelector<HTMLButtonElement>("#flashButton")!;
const writeTitle = document.querySelector<HTMLDivElement>("#writeTitle")!;
const writePercent = document.querySelector<HTMLDivElement>("#writePercent")!;
const progressFill = document.querySelector<HTMLDivElement>("#progressFill")!;
const recordTime = document.querySelector<HTMLSpanElement>("#recordTime")!;
const recordText = document.querySelector<HTMLSpanElement>("#recordText")!;
const copyLogButton = document.querySelector<HTMLButtonElement>("#copyLogButton")!;

let flashing = false;
let scanTimer: number | undefined;
let lastRecord = "";

function nowText(): string {
  return new Date().toLocaleTimeString("zh-CN", { hour12: false });
}

function setRecord(text: string, force = false): void {
  if (!force && text === lastRecord) {
    return;
  }

  lastRecord = text;
  recordTime.textContent = nowText();
  recordText.textContent = text;
}

function setPill(text: string, kind: "warn" | "ok" | "info" | "error"): void {
  statusPill.textContent = text;
  statusPill.className = `pill ${kind}`;
}

function setProgress(percent: number): void {
  const clamped = Math.max(0, Math.min(100, percent));
  progressFill.style.width = `${clamped}%`;
}

function setCopyVisible(visible: boolean): void {
  copyLogButton.classList.toggle("hidden", !visible);
}

function setWriteIdle(enabled: boolean): void {
  flashButton.textContent = "写入";
  flashButton.disabled = !enabled;
}

function renderReady(status: UiDeviceStatus): void {
  setPill("设备已连接", "ok");
  deviceName.textContent = status.device_name || "CH32x035";
  devicePort.textContent = status.port_name ?? "";
  writeTitle.textContent = "准备就绪";
  writePercent.textContent = "";
  setWriteIdle(status.button_enabled && !flashing);
  if (!flashing) {
    setProgress(0);
    setCopyVisible(false);
    setRecord("设备就绪");
  }
}

function renderNoDevice(status?: UiDeviceStatus): void {
  const title = status?.title || "未连接";
  flashing = false;
  setPill("未检测到", "warn");
  deviceName.textContent = title;
  devicePort.textContent = "";
  writeTitle.textContent = title;
  writePercent.textContent = "";
  setWriteIdle(false);
  setProgress(0);
  setCopyVisible(false);
  setRecord(title);
}

function renderOtherStatus(status: UiDeviceStatus): void {
  const title = status.title || "未连接";
  setPill(title, status.button_enabled ? "info" : "warn");
  deviceName.textContent = title;
  devicePort.textContent = status.port_name ?? "";
  writeTitle.textContent = title;
  writePercent.textContent = "";
  setWriteIdle(status.button_enabled && !flashing);
  setProgress(0);
  setCopyVisible(false);
  setRecord(title);
}

function renderStatus(status: UiDeviceStatus): void {
  switch (status.kind) {
    case "Ready":
      renderReady(status);
      return;
    case "NoDevice":
      renderNoDevice(status);
      return;
    default:
      renderOtherStatus(status);
  }
}

async function scan(): Promise<void> {
  if (flashing) {
    return;
  }

  try {
    const status = await invoke<UiDeviceStatus>("scan_devices");
    renderStatus(status);
  } catch {
    renderNoDevice();
  }
}

function renderFlashFailure(error: UiError | string): void {
  const message = typeof error === "string" ? error : error.message || "写入失败";
  flashing = false;
  setPill("失败", "error");
  writeTitle.textContent = "无法写入";
  writePercent.textContent = "";
  setWriteIdle(false);
  setCopyVisible(true);
  setProgress(0);
  setRecord(message, true);
}

async function watchEvent<T>(
  eventName: string,
  handler: Parameters<typeof listen<T>>[1],
): Promise<void> {
  try {
    await listen<T>(eventName, handler);
  } catch {
    // Allows browser-only visual checks without Tauri IPC.
  }
}

async function startFlash(): Promise<void> {
  flashing = true;
  flashButton.disabled = true;
  flashButton.textContent = "写入中";
  writeTitle.textContent = "写入中";
  writePercent.textContent = "0%";
  setPill("写入中", "info");
  setProgress(0);
  setCopyVisible(false);
  setRecord("写入中 0%", true);

  try {
    await invoke("start_flash");
  } catch (error) {
    renderFlashFailure(error as UiError | string);
  }
}

rescanButton.addEventListener("click", () => void scan());
flashButton.addEventListener("click", () => void startFlash());
copyLogButton.addEventListener("click", async () => {
  const text = await invoke<string>("copy_log");
  await navigator.clipboard.writeText(text);
  setRecord("记录已复制", true);
});

async function init(): Promise<void> {
  setRecord("未连接", true);

  await watchEvent<UiDeviceStatus>("device-status-changed", (event) => {
    if (!flashing) {
      renderStatus(event.payload);
    }
  });

  await watchEvent<FlashProgress>("flash-progress", (event) => {
    const percent = event.payload.percent;
    writeTitle.textContent = "写入中";
    writePercent.textContent = `${percent}%`;
    setProgress(percent);
    setRecord(`写入中 ${percent}%`, true);
  });

  await watchEvent<string>("flash-finished", () => {
    flashing = false;
    setPill("已完成", "ok");
    writeTitle.textContent = "完成";
    writePercent.textContent = "";
    flashButton.textContent = "重新写入";
    flashButton.disabled = false;
    setCopyVisible(true);
    setProgress(100);
    setRecord("写入完成", true);
  });

  await watchEvent<UiError>("flash-failed", (event) => {
    renderFlashFailure(event.payload);
  });

  scanTimer = window.setInterval(() => void scan(), 1000);
  void scan();
}

window.addEventListener("beforeunload", () => {
  if (scanTimer !== undefined) {
    window.clearInterval(scanTimer);
  }
});

void init();
