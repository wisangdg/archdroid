import React, { useState, useEffect } from "react";
import {
  Monitor,
  Smartphone,
  Tablet,
  Tv,
  Sliders,
  Check,
  RotateCw,
  Sparkles,
} from "lucide-react";

export const PRESETS = [
  {
    id: "tablet_1080p",
    name: "Tablet Full HD",
    width: "1920",
    height: "1080",
    dpi: "280",
    ratio: "16:9 Landscape",
    icon: Tablet,
    description:
      "Standard widescreen layout for PUBG, Free Fire, and MOBA games.",
    tag: "Recommended",
  },
  {
    id: "tablet_720p",
    name: "Standard 720p",
    width: "1280",
    height: "720",
    dpi: "240",
    ratio: "16:9 Landscape",
    icon: Tv,
    description:
      "Lightweight widescreen, ideal for high FPS or lower-end GPUs.",
  },
  {
    id: "phone_portrait",
    name: "Mobile Phone",
    width: "1080",
    height: "1920",
    dpi: "420",
    ratio: "9:16 Portrait",
    icon: Smartphone,
    description:
      "Vertical phone format, ideal for TikTok, Reels, and portrait games.",
  },
  {
    id: "desktop_2k",
    name: "Desktop 2K / QHD",
    width: "2560",
    height: "1440",
    dpi: "320",
    ratio: "16:9 High-Res",
    icon: Monitor,
    description: "Ultra-crisp resolution for large 1440p+ desktop monitors.",
  },
  {
    id: "custom",
    name: "Custom Resolution",
    width: "",
    height: "",
    dpi: "",
    ratio: "Bespoke",
    icon: Sliders,
    description: "Set your own custom pixel dimensions and pixel density.",
  },
];

const COMMON_DPIS = [
  { val: "160", label: "160 (mdpi)" },
  { val: "240", label: "240 (hdpi)" },
  { val: "280", label: "280 (tablet)" },
  { val: "320", label: "320 (xhdpi)" },
  { val: "420", label: "420 (phone)" },
  { val: "480", label: "480 (xxhdpi)" },
];

export default function ResolutionPresets({
  currentWidth = "",
  currentHeight = "",
  currentDpi = "",
  onApplyResolution,
  engineRunning = false,
  onRestartEngine,
  disabled = false,
}) {
  const [selectedPresetId, setSelectedPresetId] = useState("tablet_1080p");
  const [customWidth, setCustomWidth] = useState(currentWidth || "1920");
  const [customHeight, setCustomHeight] = useState(currentHeight || "1080");
  const [customDpi, setCustomDpi] = useState(currentDpi || "280");
  const [isApplying, setIsApplying] = useState(false);

  // Sync with current props on initial load or change
  useEffect(() => {
    if (currentWidth && currentHeight) {
      const match = PRESETS.find(
        (p) =>
          p.id !== "custom" &&
          p.width === currentWidth &&
          p.height === currentHeight &&
          (!p.dpi || p.dpi === currentDpi)
      );
      if (match) {
        setSelectedPresetId(match.id);
      } else {
        setSelectedPresetId("custom");
        setCustomWidth(currentWidth);
        setCustomHeight(currentHeight);
        setCustomDpi(currentDpi || "280");
      }
    }
  }, [currentWidth, currentHeight, currentDpi]);

  const handleSelectPreset = (preset) => {
    setSelectedPresetId(preset.id);
    if (preset.id !== "custom") {
      setCustomWidth(preset.width);
      setCustomHeight(preset.height);
      setCustomDpi(preset.dpi);
    }
  };

  const isCurrentActive = (preset) => {
    if (!currentWidth || !currentHeight) return false;
    if (preset.id === "custom") {
      return !PRESETS.some(
        (p) =>
          p.id !== "custom" &&
          p.width === currentWidth &&
          p.height === currentHeight
      );
    }
    return (
      preset.width === currentWidth &&
      preset.height === currentHeight &&
      (!preset.dpi || preset.dpi === currentDpi)
    );
  };

  const handleApply = async () => {
    setIsApplying(true);
    try {
      await onApplyResolution({
        width: customWidth.trim(),
        height: customHeight.trim(),
        dpi: customDpi.trim(),
      });
    } finally {
      setIsApplying(false);
    }
  };

  const isUnchanged =
    customWidth === currentWidth &&
    customHeight === currentHeight &&
    (customDpi === currentDpi || (!customDpi && !currentDpi));

  return (
    <div className="glass-panel resolution-presets-panel">
      {/* Panel Header */}
      <div className="section-title-wrap">
        <Monitor size={22} className="section-title-icon" />
        <div style={{ flex: 1 }}>
          <div className="res-title-row">
            <h3>Display & Resolution Presets</h3>
            {/* Active Display Indicator Badge */}
            <span className="current-res-badge">
              Active:{" "}
              <strong>
                {currentWidth && currentHeight
                  ? `${currentWidth} × ${currentHeight}`
                  : "Auto / Default"}
              </strong>
              {currentDpi && (
                <span className="res-dpi-tag">{currentDpi} DPI</span>
              )}
            </span>
          </div>
          <p>
            Choose a visual display format optimized for mobile gaming, tablet
            multi-tasking, or desktop streaming.
          </p>
        </div>
      </div>

      {/* Preset Cards Grid */}
      <div className="presets-grid">
        {PRESETS.map((preset) => {
          const Icon = preset.icon;
          const isSelected = selectedPresetId === preset.id;
          const isActive = isCurrentActive(preset);

          return (
            <div
              key={preset.id}
              className={`preset-card ${isSelected ? "selected" : ""} ${
                isActive ? "active-profile" : ""
              }`}
              onClick={() => handleSelectPreset(preset)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleSelectPreset(preset);
                }
              }}
              role="button"
              tabIndex={0}
              aria-pressed={isSelected}
              aria-label={`${preset.name} (${preset.width}x${preset.height})`}
            >
              <div className="preset-card-top">
                <div className="preset-icon-wrapper">
                  <Icon size={20} aria-hidden="true" />
                </div>
                <div className="preset-badges">
                  {preset.tag && (
                    <span className="preset-badge-tag">
                      <Sparkles size={11} aria-hidden="true" />
                      {preset.tag}
                    </span>
                  )}
                  {isActive && (
                    <span className="preset-badge-active">
                      <Check size={11} aria-hidden="true" />
                      {/* Current */}
                      Current
                    </span>
                  )}
                </div>
              </div>

              <div className="preset-info">
                <h4 className="preset-name">{preset.name}</h4>
                <div className="preset-resolution">
                  {preset.id !== "custom" ? (
                    <>
                      <span>
                        {preset.width} × {preset.height}
                      </span>
                      <span className="res-sep">•</span>
                      <span className="preset-dpi">{preset.dpi} DPI</span>
                    </>
                  ) : (
                    <span className="preset-custom-label">
                      Bespoke dimensions
                    </span>
                  )}
                </div>
                <p className="preset-description">{preset.description}</p>
              </div>

              <div className="preset-card-footer">
                <span className="preset-ratio-pill">{preset.ratio}</span>
                <div className={`preset-radio ${isSelected ? "checked" : ""}`}>
                  {isSelected && <div className="preset-radio-dot" />}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Custom Inputs Form (Visible when Custom is selected or to refine values) */}
      {selectedPresetId === "custom" && (
        <div className="custom-res-form-panel">
          <h5 className="custom-res-title">
            Custom Dimensions & Pixel Density
          </h5>
          <div className="custom-res-inputs">
            <div className="res-input-group">
              <label>Width (px)</label>
              <input
                type="number"
                min="480"
                max="7680"
                step="10"
                value={customWidth}
                onChange={(e) => setCustomWidth(e.target.value)}
                placeholder="e.g. 1920"
                className="res-number-input"
              />
            </div>

            <span className="res-times-symbol">×</span>

            <div className="res-input-group">
              <label>Height (px)</label>
              <input
                type="number"
                min="480"
                max="4320"
                step="10"
                value={customHeight}
                onChange={(e) => setCustomHeight(e.target.value)}
                placeholder="e.g. 1080"
                className="res-number-input"
              />
            </div>

            <div className="res-input-group">
              <label>Density (DPI)</label>
              <input
                type="number"
                min="120"
                max="640"
                step="10"
                value={customDpi}
                onChange={(e) => setCustomDpi(e.target.value)}
                placeholder="e.g. 280"
                className="res-number-input"
              />
            </div>
          </div>

          {/* Quick DPI Chips */}
          <div className="quick-dpi-chips">
            <span className="quick-dpi-label">Preset DPIs:</span>
            {COMMON_DPIS.map((item) => (
              <button
                key={item.val}
                type="button"
                className={`quick-dpi-btn ${customDpi === item.val ? "active" : ""}`}
                onClick={() => setCustomDpi(item.val)}
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Action Bar */}
      <div className="res-apply-bar">
        <div className="res-apply-notice">
          <p>
            ℹ️ Changes to display resolution and DPI require a{" "}
            <strong>restart of the Waydroid session</strong> to take visual
            effect in Android.
          </p>
        </div>

        <div className="res-actions-right">
          {engineRunning && !isUnchanged && (
            <button
              type="button"
              className="btn btn-outline btn-sm"
              onClick={onRestartEngine}
              disabled={disabled}
              title="Restart session immediately"
            >
              <RotateCw size={14} />
              <span>Restart Engine</span>
            </button>
          )}

          <button
            type="button"
            className="btn btn-primary btn-apply-res"
            onClick={handleApply}
            disabled={
              disabled ||
              isApplying ||
              isUnchanged ||
              !customWidth ||
              !customHeight
            }
          >
            <Check size={16} />
            <span>
              {isApplying
                ? "Applying..."
                : isUnchanged
                  ? "Saved & Active"
                  : `Apply ${customWidth} × ${customHeight}`}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
