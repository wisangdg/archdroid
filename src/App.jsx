import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
	MonitorPlay,
	Settings,
	Download,
	Gamepad2,
	Play,
	Square,
	Plus,
	FolderOpen,
	Trash2,
} from "lucide-react";

function App() {
	const [status, setStatus] = useState("offline");
	const [isInitialized, setIsInitialized] = useState(true);
	const [activeTab, setActiveTab] = useState("home");
	const [apps, setApps] = useState([]);
	const [gappsStatus, setGappsStatus] = useState("unknown"); // 'unknown' | 'installed' | 'partial' | 'not_installed'
	const [houdiniStatus, setHoudiniStatus] = useState("unknown"); // 'unknown' | 'installed' | 'not_installed'
	const [installing, setInstalling] = useState(false);
	const prevStatusRef = useRef("offline");

	// Terminal Modal State
	const [terminalOpen, setTerminalOpen] = useState(false);
	const [terminalLogs, setTerminalLogs] = useState([]);
	const [terminalTitle, setTerminalTitle] = useState("");
	const terminalRef = useRef(null);

	// Config State
	const [multiWindow, setMultiWindow] = useState(false);
	const [invertColors, setInvertColors] = useState(false);

	useEffect(() => {
		checkStatus();
		loadApps();
		checkGappsStatus();
		checkHoudiniStatus();
		loadConfigs();
		const interval = setInterval(() => {
			checkStatus();
			checkGappsStatus();
			checkHoudiniStatus();
		}, 5000);

		const unlistenOutput = listen("cmd-output", (event) => {
			setTerminalLogs(prev => [...prev, event.payload]);
			if (terminalRef.current) {
				terminalRef.current.scrollTop = terminalRef.current.scrollHeight;
			}
		});

		const unlistenFinished = listen("cmd-finished", (event) => {
			const success = event.payload;
			setTerminalLogs(prev => [...prev, success ? "\n[PROCESS COMPLETED SUCCESSFULLY]" : "\n[PROCESS FINISHED WITH ERRORS]"]);
			setInstalling(false);
		});

		return () => {
			clearInterval(interval);
			unlistenOutput.then(f => f());
			unlistenFinished.then(f => f());
		};
	}, []);

	useEffect(() => {
		prevStatusRef.current = status;
	}, [status]);

	const checkGappsStatus = async () => {
		try {
			const res = await invoke("check_gapps_status");
			setGappsStatus(res.status);
		} catch (e) {
			console.error(e);
			setGappsStatus("unknown");
		}
	};

	const checkHoudiniStatus = async () => {
		try {
			const res = await invoke("check_houdini_status");
			setHoudiniStatus(res.status);
		} catch (e) {
			console.error(e);
		}
	};

	const loadApps = async () => {
		try {
			const waydroidApps = await invoke("get_waydroid_apps");
			setApps(waydroidApps);
		} catch (e) {
			console.error(e);
		}
	};

	const checkStatus = async () => {
		try {
			const res = await invoke("waydroid_status");
			const prevStatus = status;
			if (
				res.status === "success" &&
				res.data &&
				res.data.includes("RUNNING")
			) {
				setStatus("online");
				setIsInitialized(true);
				// Re-check GApps when Waydroid comes online
				if (prevStatusRef.current !== "online") {
					checkGappsStatus();
					checkHoudiniStatus();
				}
			} else {
				setStatus("offline");
				if (
					res.message &&
					(res.message.includes("Cannot locate waydroid config") ||
						res.message.includes("command not found") ||
						res.message.includes("No such file"))
				) {
					setIsInitialized(false);
				} else {
					setIsInitialized(true);
				}
			}
		} catch (e) {
			console.error(e);
			setStatus("offline");
		}
	};

	const handleStart = async () => {
		try {
			await invoke("waydroid_start");
			setTimeout(checkStatus, 2000);
		} catch (e) {
			console.error(e);
		}
	};

	const handleStop = async () => {
		try {
			await invoke("waydroid_stop");
			setTimeout(checkStatus, 2000);
		} catch (e) {
			console.error(e);
		}
	};

	const handleInstallApk = async () => {
		try {
			const file = await open({
				multiple: false,
				filters: [
					{
						name: "Android Package",
						extensions: ["apk"],
					},
				],
			});
			if (file) {
				alert("Installing APK, this may take a moment...");
				const res = await invoke("waydroid_install_apk", {
					path: file,
				});
				if (res.status === "success") {
					alert("APK Installed successfully!");
					loadApps();
				} else {
					alert("Failed to install APK: " + res.message);
				}
			}
		} catch (e) {
			console.error(e);
		}
	};

	const handleLaunchApp = async (pkg) => {
		try {
			await invoke("waydroid_launch_app", { package: pkg });
		} catch (e) {
			console.error(e);
		}
	};

	const handleInstallGApps = async () => {
		try {
			setInstalling(true);
			setTerminalTitle("Installing GApps...");
			setTerminalLogs([]);
			setTerminalOpen(true);
			await invoke("install_gapps");
		} catch (e) {
			console.error(e);
			setInstalling(false);
		}
	};

	const handleInstallHoudini = async () => {
		try {
			setInstalling(true);
			setTerminalTitle("Installing ARM Translation (libhoudini)...");
			setTerminalLogs([]);
			setTerminalOpen(true);
			await invoke("install_houdini");
		} catch (e) {
			console.error(e);
			setInstalling(false);
		}
	};

	const handleUninstallApp = async (e, pkg) => {
		e.stopPropagation();
		if (window.confirm(`Are you sure you want to uninstall ${pkg}?`)) {
			try {
				await invoke("waydroid_uninstall_app", { package: pkg });
				await loadApps();
			} catch (err) {
				console.error(err);
			}
		}
	};

	const handleViewLogs = async () => {
		try {
			setInstalling(true);
			setTerminalTitle("Waydroid System Logs");
			setTerminalLogs([]);
			setTerminalOpen(true);
			await invoke("waydroid_log");
		} catch (e) {
			console.error(e);
			setInstalling(false);
		}
	};

	const handleOpenSharedFolder = async () => {
		try {
			await invoke("open_shared_folder");
		} catch (e) {
			console.error(e);
		}
	};

	const loadConfigs = async () => {
		try {
			const mw = await invoke("waydroid_prop_get", { key: "persist.waydroid.multi_windows" });
			if (mw.status === "success" && mw.data === "true") setMultiWindow(true);
			else setMultiWindow(false);

			const inv = await invoke("waydroid_prop_get", { key: "persist.waydroid.invert_colors" });
			if (inv.status === "success" && inv.data === "true") setInvertColors(true);
			else setInvertColors(false);
		} catch (e) {
			console.error(e);
		}
	};

	const toggleConfig = async (key, currentValue, setter) => {
		const newValue = currentValue ? "false" : "true";
		try {
			await invoke("waydroid_prop_set", { key, value: newValue });
			setter(!currentValue);
		} catch (e) {
			console.error("Failed to toggle config", e);
		}
	};

	const handlePropSet = async (e) => {
		e.preventDefault();
		const key = e.target.elements.propKey.value;
		const val = e.target.elements.propVal.value;
		try {
			await invoke("waydroid_prop_set", { key: key, value: val });
			e.target.reset();
			await loadConfigs(); // Reload toggles if they match
		} catch (err) {
			console.error(err);
		}
	};

	const handleInstallPhantom = async () => {
		try {
			setInstalling(true);
			setTerminalTitle("Installing Phantom Keymapper...");
			setTerminalLogs([]);
			setTerminalOpen(true);
			await invoke("install_phantom");
		} catch (e) {
			console.error(e);
			setInstalling(false);
		}
	};

	const handleRunPhantom = async () => {
		try {
			await invoke("run_phantom");
		} catch (e) {
			console.error(e);
		}
	};

	const handleInitWaydroid = async () => {
		try {
			setInstalling(true);
			setTerminalTitle("Initializing Waydroid...");
			setTerminalLogs([]);
			setTerminalOpen(true);
			await invoke("waydroid_init");
		} catch (e) {
			console.error(e);
			setInstalling(false);
		}
	};

	const handleWipeData = async () => {
		if (window.confirm("Are you sure you want to completely wipe ALL Waydroid data, apps, and GApps? This action cannot be undone and will require a full re-initialization!")) {
			try {
				setInstalling(true);
				setTerminalTitle("Factory Reset...");
				setTerminalLogs([]);
				setTerminalOpen(true);
				await invoke("waydroid_wipe");
			} catch (e) {
				console.error(e);
				setInstalling(false);
			}
		}
	};


	return (
		<div className="app-container">
			{/* Sidebar */}
			<div className="sidebar">
				<div className="brand">
					<MonitorPlay size={28} className="icon-glow" />
					<span>CachyDroid</span>
				</div>

				<div style={{ marginTop: "20px" }}>
					<button
						className={`menu-item ${activeTab === "home" ? "active" : ""}`}
						onClick={() => setActiveTab("home")}
					>
						<MonitorPlay size={20} /> My Apps
					</button>
					<button
						className={`menu-item ${activeTab === "keymap" ? "active" : ""}`}
						onClick={() => setActiveTab("keymap")}
					>
						<Gamepad2 size={20} /> Keymapping
					</button>
					<button
						className={`menu-item ${activeTab === "install" ? "active" : ""}`}
						onClick={() => setActiveTab("install")}
					>
						<Download size={20} /> Install Tools
					</button>
					<button
						className={`menu-item ${activeTab === "settings" ? "active" : ""}`}
						onClick={() => setActiveTab("settings")}
					>
						<Settings size={20} /> Settings
					</button>
				</div>
			</div>

			{/* Main Content */}
			<div className="main-content">
				<div className="header">
					<h1>
						{activeTab === "home"
							? "My Applications"
							: activeTab === "keymap"
								? "Keymapping Setup"
								: activeTab === "install"
									? "Install Tools"
									: "Settings"}
					</h1>

					<div style={{ display: "flex", gap: "15px", alignItems: "center" }}>
						<div className={`status-badge ${status === "online" ? "status-online" : "status-offline"}`}>
							<div className="status-dot"></div>
							{status === "online" ? "Engine Running" : "Engine Stopped"}
						</div>

						{isInitialized &&
							(status === "offline" ? (
								<button className="btn btn-primary" onClick={handleStart}>
									<Play size={18} fill="currentColor" /> Start
								</button>
							) : (
								<button className="btn btn-danger" onClick={handleStop}>
									<Square size={18} fill="currentColor" /> Stop
								</button>
							))}
					</div>
				</div>

				{!isInitialized ? (
					<div className="glass-panel" style={{ textAlign: "center", padding: "80px 20px", marginTop: "40px" }}>
						<div style={{ display: "inline-block", position: "relative", marginBottom: "30px" }}>
							<MonitorPlay size={80} className="icon-glow icon-float" style={{ color: "var(--accent-primary)" }} />
							<div style={{ position: "absolute", inset: "-20px", background: "radial-gradient(circle, var(--accent-glow) 0%, transparent 70%)", zIndex: -1, borderRadius: "50%" }}></div>
						</div>
						<h2>First Time Setup Required</h2>
						<p style={{ color: "var(--text-secondary)", maxWidth: "550px", margin: "0 auto 40px auto", fontSize: "16px" }}>
							Your Linux system is ready, but the Android OS image is missing. Click the button below to initialize Waydroid. This will download the core system files (~1GB) and set up the emulator perfectly for you.
						</p>
						<button className="btn btn-primary" onClick={handleInitWaydroid} style={{ padding: "14px 28px", fontSize: "16px" }}>
							<Download size={20} /> Initialize Emulator
						</button>
					</div>
				) : (
					<>
						{activeTab === "home" && (
							<div className="app-grid">
								{apps.map((app) => (
									<div className="app-card relative-card" key={app.id} onClick={() => handleLaunchApp(app.package)} style={{ position: "relative" }}>
										<div className="app-icon" style={{ overflow: "hidden" }}>
											{app.icon ? (
												<img src={app.icon} alt={app.name} style={{ width: "100%", height: "100%", objectFit: "cover" }} />
											) : (
												"🤖"
											)}
										</div>
										<div className="app-name">{app.name}</div>
										
										{/* Uninstall Button overlay on hover */}
										<button 
											className="btn-uninstall-overlay"
											onClick={(e) => handleUninstallApp(e, app.package)}
											title="Uninstall App"
										>
											<Trash2 size={16} />
										</button>
									</div>
								))}
								<div className="app-card dashed" onClick={handleInstallApk}>
									<div className="app-icon" style={{ background: "transparent", border: "none", boxShadow: "none" }}>
										<Plus size={36} color="var(--accent-primary)" />
									</div>
									<div className="app-name" style={{ color: "var(--accent-primary)" }}>Install APK</div>
								</div>
							</div>
						)}

						{activeTab === "install" && (
							<div style={{ display: "flex", flexDirection: "column", gap: "24px" }}>
								<div className="glass-panel">
									<div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
										<div>
											<h2>Install GApps</h2>
											<p style={{ margin: 0 }}>Install Google Play Store and Google Services.</p>
										</div>
										<button 
											className={gappsStatus === "installed" ? "btn btn-outline" : "btn btn-primary"} 
											onClick={handleInstallGApps} 
											disabled={installing || gappsStatus === "installed"}
										>
											<Download size={18} /> {gappsStatus === "installed" ? "Installed" : "Install GApps"}
										</button>
									</div>
									
									<div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: "20px" }}>
										<div>
											<h2>Install ARM Translation</h2>
											<p style={{ margin: 0 }}>Required to run ARM apps on x86 processors (libhoudini).</p>
										</div>
										<button 
											className={houdiniStatus === "installed" ? "btn btn-outline" : "btn btn-primary"} 
											onClick={handleInstallHoudini} 
											disabled={installing || houdiniStatus === "installed"}
										>
											<Download size={18} /> {houdiniStatus === "installed" ? "Installed" : "Install ARM Support"}
										</button>
									</div>
								</div>
							</div>
						)}

						{activeTab === "settings" && (
							<div style={{ display: "flex", flexDirection: "column", gap: "24px" }}>
								<div className="glass-panel">
									<div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
										<div>
											<h2>Shared Folder</h2>
											<p style={{ margin: 0 }}>Open the PC-Emulator shared folder to easily transfer APKs, OBBs, or media files.</p>
										</div>
										<button className="btn btn-outline" onClick={handleOpenSharedFolder}>
											<FolderOpen size={18} /> Open Directory
										</button>
									</div>
								</div>

								<div className="glass-panel">
									<h2>Visual Config Manager</h2>
									<p style={{ marginBottom: "20px" }}>Easily toggle standard Waydroid configurations.</p>
									
									<div style={{ display: "flex", flexDirection: "column", gap: "15px" }}>
										<div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "12px", background: "rgba(255,255,255,0.05)", borderRadius: "8px" }}>
											<div>
												<h4 style={{ margin: 0, fontSize: "16px" }}>Multi-Window Mode</h4>
												<p style={{ margin: 0, fontSize: "13px", color: "var(--text-secondary)" }}>Run Android apps in free-floating windows instead of full-screen UI.</p>
											</div>
											<button 
												className={multiWindow ? "btn btn-primary" : "btn btn-outline"} 
												onClick={() => toggleConfig("persist.waydroid.multi_windows", multiWindow, setMultiWindow)}
											>
												{multiWindow ? "ON" : "OFF"}
											</button>
										</div>

										<div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "12px", background: "rgba(255,255,255,0.05)", borderRadius: "8px" }}>
											<div>
												<h4 style={{ margin: 0, fontSize: "16px" }}>Invert Colors</h4>
												<p style={{ margin: 0, fontSize: "13px", color: "var(--text-secondary)" }}>Invert all Android UI colors (Useful for force-dark mode).</p>
											</div>
											<button 
												className={invertColors ? "btn btn-primary" : "btn btn-outline"} 
												onClick={() => toggleConfig("persist.waydroid.invert_colors", invertColors, setInvertColors)}
											>
												{invertColors ? "ON" : "OFF"}
											</button>
										</div>
									</div>
								</div>

								<div className="glass-panel">
									<h3>Advanced Properties (Manual)</h3>
									<form onSubmit={handlePropSet} style={{ display: "flex", gap: "15px", marginTop: "15px" }}>
										<input name="propKey" placeholder="Property Key (e.g. persist.waydroid.width)" style={{ width: "300px", padding: "10px", borderRadius: "8px", border: "1px solid var(--border-light)", background: "rgba(0,0,0,0.2)", color: "white" }} required />
										<input name="propVal" placeholder="Value" style={{ width: "150px", padding: "10px", borderRadius: "8px", border: "1px solid var(--border-light)", background: "rgba(0,0,0,0.2)", color: "white" }} required />
										<button type="submit" className="btn btn-primary">Apply</button>
									</form>
								</div>

								<div className="glass-panel">
									<div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
										<div>
											<h2>Diagnostic Logs</h2>
											<p style={{ margin: 0 }}>View internal Waydroid system logs to troubleshoot crashes or bugs.</p>
										</div>
										<button className="btn btn-outline" onClick={handleViewLogs}>
											View Logs
										</button>
									</div>
								</div>

								<div className="glass-panel" style={{ border: '1px solid rgba(239, 68, 68, 0.3)', background: 'linear-gradient(180deg, rgba(239,68,68,0.05) 0%, transparent 100%)' }}>
									<h2 style={{ color: 'var(--danger-primary)' }}>Danger Zone</h2>
									<p style={{ marginBottom: '20px' }}>Completely wipe all Waydroid data, installed apps, GApps, and configurations. This restores the emulator to a completely clean state.</p>
									<button className="btn btn-danger" onClick={handleWipeData}>
										Factory Reset (Wipe All Data)
									</button>
								</div>
							</div>
						)}

						{activeTab === "keymap" && (
							<div className="glass-panel" style={{ textAlign: "center", padding: "80px 20px" }}>
								<div style={{ display: "inline-block", position: "relative", marginBottom: "30px" }}>
									<Gamepad2 size={80} className="icon-glow icon-float" style={{ color: "var(--accent-primary)" }} />
								</div>
								<h2>Smart Keymapping</h2>
								<p style={{ color: "var(--text-secondary)", maxWidth: "550px", margin: "0 auto 40px auto", fontSize: "16px" }}>
									Map your keyboard and mouse to touch screen inputs for the ultimate gaming experience using <strong>Phantom</strong> (a native Linux keymapper for Waydroid).
								</p>
								<div style={{ display: "flex", gap: "20px", justifyContent: "center" }}>
									<button className="btn btn-primary" onClick={handleRunPhantom}>
										Launch Keymapper
									</button>
									<button className="btn btn-outline" onClick={handleInstallPhantom}>
										Install / Update Phantom
									</button>
								</div>
							</div>
						)}
					</>
				)}
			</div>

			{/* Terminal Modal Overlay */}
			{terminalOpen && (
				<div style={{
					position: "fixed", top: 0, left: 0, right: 0, bottom: 0,
					background: "rgba(0,0,0,0.8)", backdropFilter: "blur(5px)",
					display: "flex", justifyContent: "center", alignItems: "center", zIndex: 100
				}}>
					<div className="glass-panel" style={{ width: "80%", maxWidth: "900px", display: "flex", flexDirection: "column", padding: "20px" }}>
						<div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "15px" }}>
							<h3 style={{ margin: 0 }}>{terminalTitle}</h3>
							{!installing && (
								<button className="btn btn-outline" onClick={() => setTerminalOpen(false)} style={{ padding: "8px 16px" }}>
									Close
								</button>
							)}
						</div>
						<div ref={terminalRef} style={{
							background: "#0d1117", padding: "15px", borderRadius: "8px",
							height: "400px", overflowY: "auto", fontFamily: "monospace", fontSize: "13px",
							color: "#c9d1d9", border: "1px solid #30363d", whiteSpace: "pre-wrap"
						}}>
							{terminalLogs.length === 0 ? "Initializing..." : terminalLogs.join("\n")}
						</div>
					</div>
				</div>
			)}
		</div>
	);
}

export default App;

