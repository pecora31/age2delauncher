let isRunning = false;
let currentPrefs = null;

// Civilization & Adaptive Theme Configuration
const CIV_CONFIGS = {
  empires: {
    name: "Empires",
    bg: "assets/images/default_bg.jpg",
    audio: "assets/audio/aoe4_theme.mp3"
  },
  rus: {
    name: "Rus",
    bg: "assets/images/wallpapers/rus.webp",
    audio: "assets/audio/dawn_rus.mp3"
  },
  delhi: {
    name: "Delhi Sultanate",
    bg: "assets/images/wallpapers/delhi.webp",
    audio: "assets/audio/dawn_delhi.mp3"
  },
  hre: {
    name: "Holy Roman Empire",
    bg: "assets/images/wallpapers/hre.webp",
    audio: "assets/audio/dawn_hre.mp3"
  },
  french: {
    name: "French",
    bg: "assets/images/wallpapers/french.webp",
    audio: "assets/audio/dawn_french.mp3"
  },
  mongols: {
    name: "Mongols",
    bg: "assets/images/wallpapers/mongols.webp",
    audio: "assets/audio/dawn_mongols.mp3"
  },
  abbasid: {
    name: "Abbasid Dynasty",
    bg: "assets/images/wallpapers/abbasid.webp",
    audio: "assets/audio/dawn_abbasid.mp3"
  },
  chinese: {
    name: "Chinese",
    bg: "assets/images/wallpapers/chinese.webp",
    audio: "assets/audio/dawn_chinese.mp3"
  },
  english: {
    name: "English",
    bg: "assets/images/wallpapers/english.webp",
    audio: "assets/audio/dawn_english.mp3"
  }
};

// Default to Empires background and AoE4 Main Theme on startup
let currentCiv = "empires";
let currentVolume = parseFloat(localStorage.getItem("aoe2_music_volume") || "0.70");
let isMuted = localStorage.getItem("aoe2_music_muted") === "true";

// DOM Elements
const gameDirInput = document.getElementById("gameDir");
const btnBrowse = document.getElementById("btnBrowse");
const playerNameInput = document.getElementById("playerName");
const steamIdInput = document.getElementById("steamId");
const btnGenSteamId = document.getElementById("btnGenSteamId");
const tabHost = document.getElementById("tabHost");
const tabClient = document.getElementById("tabClient");
const panelHost = document.getElementById("panelHost");
const panelClient = document.getElementById("panelClient");
const hostIpInput = document.getElementById("hostIp");
const selectDetectedIps = document.getElementById("selectDetectedIps");
const clientHostIpInput = document.getElementById("clientHostIp");
const statusText = document.getElementById("statusText");
const btnAction = document.getElementById("btnAction");
const btnActionText = document.getElementById("btnActionText");

// Dual Dissolve Background Layers
const bgLayer1 = document.getElementById("bgLayer1");
const bgLayer2 = document.getElementById("bgLayer2");
let activeBgLayer = bgLayer1;

// Dual Adaptive Audio Channels & Native Web Audio Engine
const audioA = document.getElementById("audioA");
const audioB = document.getElementById("audioB");
let activeAudio = audioA;
let inactiveAudio = audioB;
let audioCtx = null;
let sourceA = null;
let sourceB = null;
let gainA = null;
let gainB = null;
let masterGain = null;
let fadeTimeout = null;

// Volume Controls
const volumeSlider = document.getElementById("volumeSlider");
const btnMusicToggle = document.getElementById("btnMusicToggle");
const musicIconOn = document.getElementById("musicIconOn");
const musicIconOff = document.getElementById("musicIconOff");

function getTauri() {
  return window.__TAURI__ || null;
}

async function invoke(cmd, args = {}) {
  const tauri = getTauri();
  if (tauri && tauri.core) {
    return await tauri.core.invoke(cmd, args);
  }
  throw new Error("Tauri core not initialized");
}

function initAudioContext() {
  if (audioCtx) return audioCtx;
  const AudioContextClass = window.AudioContext || window.webkitAudioContext;
  audioCtx = new AudioContextClass();

  sourceA = audioCtx.createMediaElementSource(audioA);
  sourceB = audioCtx.createMediaElementSource(audioB);

  gainA = audioCtx.createGain();
  gainB = audioCtx.createGain();
  masterGain = audioCtx.createGain();

  gainA.gain.value = 1.0;
  gainB.gain.value = 0.0;
  masterGain.gain.value = isMuted ? 0.0 : currentVolume;

  sourceA.connect(gainA);
  sourceB.connect(gainB);
  gainA.connect(masterGain);
  gainB.connect(masterGain);
  masterGain.connect(audioCtx.destination);

  return audioCtx;
}

async function init() {
  try {
    currentPrefs = await invoke("load_preferences");

    gameDirInput.value = currentPrefs.game_dir || "";
    playerNameInput.value = currentPrefs.player_name || "";
    steamIdInput.value = currentPrefs.steam_id || "";

    setMode(currentPrefs.is_host);

    if (currentPrefs.is_host) {
      hostIpInput.value = currentPrefs.host_ip || "";
    } else {
      clientHostIpInput.value = currentPrefs.host_ip || "";
    }

    // Populate network IPs in under 1ms via Win32 GetIpAddrTable
    const ips = await invoke("get_available_ips");
    selectDetectedIps.innerHTML = "";
    for (const ip of ips) {
      const opt = document.createElement("option");
      opt.value = ip;
      opt.textContent = ip;
      selectDetectedIps.appendChild(opt);
    }

    const radminIp = await invoke("detect_radmin_ip");
    if (radminIp && ips.includes(radminIp)) {
      selectDetectedIps.value = radminIp;
      if (!hostIpInput.value) {
        hostIpInput.value = radminIp;
      }
    }

    await validateGamePath();

    // Check if game is already running
    checkRunningState();
    setInterval(checkRunningState, 1500);

    // Initialize Adaptive Media & Volume
    initMedia();

    // Setup Civ Flag Listeners
    initCivFlags();
  } catch (err) {
    statusText.textContent = `Init error: ${err}`;
  }
}

function initMedia() {
  volumeSlider.value = currentVolume;
  updateMusicUI(!isMuted && currentVolume > 0);

  const initialConfig = CIV_CONFIGS[currentCiv] || CIV_CONFIGS.empires;
  bgLayer1.style.backgroundImage = `url('${initialConfig.bg}')`;
  bgLayer1.classList.add("active");

  activeAudio.src = initialConfig.audio;

  // Browser Autoplay Unlock via first user gesture
  const unlockAudio = () => {
    initAudioContext();
    if (audioCtx && audioCtx.state === "suspended") {
      audioCtx.resume();
    }
    if (!isMuted && currentVolume > 0 && activeAudio.paused) {
      activeAudio.play().catch(() => {});
    }
    window.removeEventListener("click", unlockAudio);
    window.removeEventListener("keydown", unlockAudio);
  };
  window.addEventListener("click", unlockAudio);
  window.addEventListener("keydown", unlockAudio);

  if (!isMuted && currentVolume > 0) {
    initAudioContext();
    activeAudio.play().catch(() => {});
  }

  // Volume Slider Event
  volumeSlider.addEventListener("input", (e) => {
    currentVolume = parseFloat(e.target.value);
    localStorage.setItem("aoe2_music_volume", currentVolume.toString());

    if (masterGain && audioCtx) {
      masterGain.gain.setValueAtTime(isMuted ? 0 : currentVolume, audioCtx.currentTime);
    }

    if (currentVolume === 0) {
      isMuted = true;
      localStorage.setItem("aoe2_music_muted", "true");
      updateMusicUI(false);
    } else {
      isMuted = false;
      localStorage.setItem("aoe2_music_muted", "false");
      updateMusicUI(true);
      if (activeAudio.paused) {
        activeAudio.play().catch(() => {});
      }
    }
  });

  // Mute Toggle Button Event
  btnMusicToggle.addEventListener("click", () => {
    initAudioContext();
    if (audioCtx && audioCtx.state === "suspended") {
      audioCtx.resume().catch(() => {});
    }

    if (isMuted || activeAudio.paused) {
      isMuted = false;
      localStorage.setItem("aoe2_music_muted", "false");
      if (currentVolume <= 0) {
        currentVolume = 0.70;
        volumeSlider.value = 0.70;
        localStorage.setItem("aoe2_music_volume", "0.70");
      }
      if (masterGain && audioCtx) {
        masterGain.gain.setValueAtTime(currentVolume, audioCtx.currentTime);
      }
      activeAudio.play().catch(() => {});
      updateMusicUI(true);
    } else {
      isMuted = true;
      localStorage.setItem("aoe2_music_muted", "true");
      if (masterGain && audioCtx) {
        masterGain.gain.setValueAtTime(0, audioCtx.currentTime);
      }
      activeAudio.pause();
      updateMusicUI(false);
    }
  });
}

function initCivFlags() {
  document.querySelectorAll(".civ-flag-btn").forEach((btn) => {
    const civKey = btn.getAttribute("data-civ");
    if (civKey === currentCiv) {
      btn.classList.add("active");
    } else {
      btn.classList.remove("active");
    }

    btn.addEventListener("click", () => {
      setCivilization(civKey);
    });
  });
}

function setCivilization(civKey) {
  if (!CIV_CONFIGS[civKey]) return;
  currentCiv = civKey;

  // Update UI flags active state
  document.querySelectorAll(".civ-flag-btn").forEach((btn) => {
    if (btn.getAttribute("data-civ") === civKey) {
      btn.classList.add("active");
    } else {
      btn.classList.remove("active");
    }
  });

  const config = CIV_CONFIGS[civKey];
  // 1. Cross-fade Background Layer
  crossfadeBackground(config.bg);
  // 2. High-precision Equal-Power Audio Transition
  crossfadeAudio(config.audio);
}

function crossfadeBackground(newBgUrl) {
  const nextLayer = (activeBgLayer === bgLayer1) ? bgLayer2 : bgLayer1;
  nextLayer.style.backgroundImage = `url('${newBgUrl}')`;
  nextLayer.classList.add("active");
  activeBgLayer.classList.remove("active");
  activeBgLayer = nextLayer;
}

// True Equal-Power Horizontal State Cross-fade via Web Audio API
function crossfadeAudio(newAudioSrc) {
  if (activeAudio.src.endsWith(newAudioSrc) && !activeAudio.paused) {
    return;
  }

  const ctx = initAudioContext();
  if (ctx.state === "suspended") {
    ctx.resume().catch(() => {});
  }

  const outgoingAudio = activeAudio;
  const incomingAudio = inactiveAudio;
  const outgoingGain = (activeAudio === audioA) ? gainA : gainB;
  const incomingGain = (activeAudio === audioA) ? gainB : gainA;

  // Swap active pointers
  activeAudio = incomingAudio;
  inactiveAudio = outgoingAudio;

  incomingAudio.src = newAudioSrc;
  incomingAudio.currentTime = 0;

  const now = ctx.currentTime;
  const duration = 1.8; // 1.8s Equal-Power Musical Transition
  const steps = 32;
  const outCurve = new Float32Array(steps);
  const inCurve = new Float32Array(steps);

  // Equal-power cosine/sine distribution (cos^2 + sin^2 = 1.0 at all points)
  for (let i = 0; i < steps; i++) {
    const ratio = i / (steps - 1);
    const rad = ratio * Math.PI * 0.5;
    outCurve[i] = Math.cos(rad);
    inCurve[i] = Math.sin(rad);
  }

  // Cancel any scheduled transitions
  outgoingGain.gain.cancelScheduledValues(now);
  incomingGain.gain.cancelScheduledValues(now);

  outgoingGain.gain.setValueCurveAtTime(outCurve, now, duration);
  incomingGain.gain.setValueCurveAtTime(inCurve, now, duration);

  const playPromise = incomingAudio.play();
  if (playPromise !== undefined) {
    playPromise.catch(() => {});
  }

  if (fadeTimeout) clearTimeout(fadeTimeout);
  fadeTimeout = setTimeout(() => {
    if (outgoingAudio !== activeAudio) {
      outgoingAudio.pause();
      outgoingAudio.currentTime = 0;
    }
  }, (duration + 0.1) * 1000);
}

function updateMusicUI(isPlaying) {
  if (isPlaying) {
    musicIconOn.classList.remove("hidden");
    musicIconOff.classList.add("hidden");
  } else {
    musicIconOn.classList.add("hidden");
    musicIconOff.classList.remove("hidden");
  }
}

function setMode(isHost) {
  if (currentPrefs) {
    currentPrefs.is_host = isHost;
  }

  if (isHost) {
    tabHost.classList.add("active");
    tabClient.classList.remove("active");
    panelHost.classList.remove("hidden");
    panelClient.classList.add("hidden");
  } else {
    tabClient.classList.add("active");
    tabHost.classList.remove("active");
    panelClient.classList.remove("hidden");
    panelHost.classList.add("hidden");
  }
}

async function validateGamePath() {
  const path = gameDirInput.value.trim();
  if (!path) {
    statusText.textContent = "Select 'Game' directory.";
    return false;
  }

  const isValid = await invoke("validate_game_directory", { path });
  if (!isValid) {
    statusText.textContent = "AoE2DE_s.exe not found in selected directory.";
    return false;
  }

  if (!isRunning) {
    statusText.textContent = "";
  }
  return true;
}

function setControlsEnabled(enabled) {
  btnBrowse.disabled = !enabled;
  playerNameInput.disabled = !enabled;
  steamIdInput.disabled = !enabled;
  btnGenSteamId.disabled = !enabled;
  tabHost.disabled = !enabled;
  tabClient.disabled = !enabled;
  hostIpInput.disabled = !enabled;
  selectDetectedIps.disabled = !enabled;
  clientHostIpInput.disabled = !enabled;
}

function setActionState(running) {
  isRunning = running;
  if (running) {
    btnActionText.textContent = "STOP GAME";
    btnAction.classList.add("btn-running");
    setControlsEnabled(false);
    statusText.textContent = "Game session active.";
  } else {
    btnActionText.textContent = "START GAME";
    btnAction.classList.remove("btn-running");
    setControlsEnabled(true);
    statusText.textContent = "";
    validateGamePath();
  }
}

async function checkRunningState() {
  try {
    const running = await invoke("is_game_running");
    if (isRunning && !running) {
      await invoke("stop_game");
      setActionState(false);
    }
  } catch {
  }
}

// Form Event Listeners
btnBrowse.addEventListener("click", async () => {
  try {
    const selected = await invoke("pick_game_directory");
    if (selected) {
      gameDirInput.value = selected;
      await validateGamePath();
    }
  } catch (err) {
    console.error("Browse error:", err);
  }
});

btnGenSteamId.addEventListener("click", async () => {
  try {
    const newId = await invoke("generate_steam_id");
    steamIdInput.value = newId;
  } catch (err) {
    statusText.textContent = `Error: ${err}`;
  }
});

tabHost.addEventListener("click", () => setMode(true));
tabClient.addEventListener("click", () => setMode(false));

selectDetectedIps.addEventListener("change", () => {
  if (selectDetectedIps.value) {
    hostIpInput.value = selectDetectedIps.value;
  }
});

btnAction.addEventListener("click", async () => {
  if (isRunning) {
    btnAction.disabled = true;
    statusText.textContent = "Stopping game...";
    try {
      await invoke("stop_game");
      setActionState(false);
    } catch (err) {
      statusText.textContent = `Stop error: ${err}`;
    } finally {
      btnAction.disabled = false;
    }
    return;
  }

  const isPathValid = await validateGamePath();
  if (!isPathValid) {
    try {
      const selected = await invoke("pick_game_directory");
      if (selected) {
        gameDirInput.value = selected;
        const valid = await validateGamePath();
        if (!valid) return;
      } else {
        return;
      }
    } catch {
      return;
    }
  }

  const isHost = tabHost.classList.contains("active");
  const hostIp = isHost ? hostIpInput.value.trim() : clientHostIpInput.value.trim();

  if (!hostIp) {
    statusText.textContent = isHost ? "Please enter/select Host IP." : "Please enter Host Radmin IP.";
    return;
  }

  const prefs = {
    game_dir: gameDirInput.value.trim(),
    player_name: playerNameInput.value.trim() || "Player",
    steam_id: steamIdInput.value.trim() || "76561198000000000",
    is_host: isHost,
    host_ip: hostIp,
  };

  btnAction.disabled = true;
  statusText.textContent = isHost ? "Starting server & game..." : "Connecting & starting game...";

  try {
    await invoke("start_game", { prefs });
    setActionState(true);
  } catch (err) {
    statusText.textContent = `Error: ${err}`;
    setActionState(false);
    alert(`Cannot start game:\n\n${err}`);
  } finally {
    btnAction.disabled = false;
  }
});

// Bootstrap
window.addEventListener("DOMContentLoaded", init);
