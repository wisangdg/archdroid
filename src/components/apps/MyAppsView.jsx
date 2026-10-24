import React, { useState, useMemo, useRef, useEffect } from "react";
import { Plus, Search, X, PackagePlus, AlertCircle, Filter } from "lucide-react";
import AppCard from "./AppCard";
import ContextMenu from "./ContextMenu";

const SYSTEM_PREFIXES = [
  "com.android.",
  "com.google.android.",
  "org.lineageos.",
  "com.google.android.inputmethod",
];

function isSystemApp(pkg) {
  return SYSTEM_PREFIXES.some((prefix) => pkg.startsWith(prefix));
}

export default function MyAppsView({
  apps,
  onLaunchApp,
  onInstallApk,
  onUninstallApp,
  onCopyPackage,
}) {
  const [searchQuery, setSearchQuery] = useState("");
  const [categoryFilter, setCategoryFilter] = useState("all"); // 'all' | 'user' | 'system'
  const [contextMenu, setContextMenu] = useState(null); // { position: { x, y }, app }
  const searchInputRef = useRef(null);

  // Global hotkey: Ctrl+F to focus search
  useEffect(() => {
    const handleKeyDown = (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchInputRef.current?.focus();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const counts = useMemo(() => {
    let user = 0;
    let system = 0;
    apps.forEach((app) => {
      if (isSystemApp(app.package)) system++;
      else user++;
    });
    return { all: apps.length, user, system };
  }, [apps]);

  const filteredApps = useMemo(() => {
    let list = apps;

    if (categoryFilter === "user") {
      list = list.filter((app) => !isSystemApp(app.package));
    } else if (categoryFilter === "system") {
      list = list.filter((app) => isSystemApp(app.package));
    }

    if (!searchQuery.trim()) return list;

    const query = searchQuery.toLowerCase().trim();
    return list.filter(
      (app) =>
        app.name.toLowerCase().includes(query) ||
        app.package.toLowerCase().includes(query)
    );
  }, [apps, categoryFilter, searchQuery]);

  const handleOpenContextMenu = (e, app) => {
    setContextMenu({
      position: { x: e.clientX, y: e.clientY },
      app,
    });
  };

  const handleCloseContextMenu = () => {
    setContextMenu(null);
  };

  return (
    <div className="apps-view-container">
      {/* Search & Actions Bar */}
      <div className="apps-toolbar">
        <div className="search-box">
          <Search size={18} className="search-icon" />
          <input
            ref={searchInputRef}
            type="text"
            className="search-input"
            placeholder="Search apps or packages... (Ctrl + F)"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
          {searchQuery && (
            <button
              type="button"
              className="search-clear-btn"
              onClick={() => setSearchQuery("")}
              title="Clear search"
            >
              <X size={15} />
            </button>
          )}
        </div>

        <button
          className="btn btn-primary btn-install-top"
          onClick={onInstallApk}
          title="Install an APK from your computer"
        >
          <PackagePlus size={18} />
          <span>Install APK</span>
        </button>
      </div>

      {/* Category Filter Pills Bar */}
      <div className="category-filter-bar">
        <div className="category-pills">
          <button
            type="button"
            className={`category-pill ${categoryFilter === "all" ? "active" : ""}`}
            onClick={() => setCategoryFilter("all")}
          >
            <span>All Apps</span>
            <span className="pill-count">{counts.all}</span>
          </button>
          <button
            type="button"
            className={`category-pill ${categoryFilter === "user" ? "active" : ""}`}
            onClick={() => setCategoryFilter("user")}
          >
            <span>User Apps & Games</span>
            <span className="pill-count">{counts.user}</span>
          </button>
          <button
            type="button"
            className={`category-pill ${categoryFilter === "system" ? "active" : ""}`}
            onClick={() => setCategoryFilter("system")}
          >
            <span>System & GApps</span>
            <span className="pill-count">{counts.system}</span>
          </button>
        </div>

        {searchQuery && (
          <span className="search-results-summary">
            Found {filteredApps.length} match{filteredApps.length === 1 ? "" : "es"}
          </span>
        )}
      </div>

      {/* App Grid */}
      <div className="app-grid">
        {/* Dashed Quick Install Card */}
        <div
          className="app-card dashed"
          onClick={onInstallApk}
          role="button"
          tabIndex={0}
        >
          <div className="dashed-icon-circle">
            <Plus size={32} />
          </div>
          <div className="app-meta">
            <span className="app-name highlight">Install APK</span>
            <span className="app-package">Browse .apk file</span>
          </div>
        </div>

        {/* Render App Cards */}
        {filteredApps.map((app) => (
          <AppCard
            key={app.id}
            app={app}
            onLaunch={onLaunchApp}
            onUninstall={onUninstallApp}
            onContextMenu={handleOpenContextMenu}
          />
        ))}
      </div>

      {/* Empty State when Search Matches Nothing */}
      {filteredApps.length === 0 && apps.length > 0 && (
        <div className="empty-search-state">
          <AlertCircle size={36} className="empty-icon" />
          <h3>No applications found</h3>
          <p>
            No installed apps matched "{searchQuery}" in category "
            {categoryFilter === "all"
              ? "All Apps"
              : categoryFilter === "user"
              ? "User Apps"
              : "System"}
            ".
          </p>
          <div style={{ display: "flex", gap: "10px", justifyContent: "center", marginTop: "14px" }}>
            {searchQuery && (
              <button
                className="btn btn-outline btn-sm"
                onClick={() => setSearchQuery("")}
              >
                Clear Search
              </button>
            )}
            {categoryFilter !== "all" && (
              <button
                className="btn btn-outline btn-sm"
                onClick={() => setCategoryFilter("all")}
              >
                Show All Categories
              </button>
            )}
          </div>
        </div>
      )}

      {/* Empty State when No Apps Installed At All */}
      {apps.length === 0 && (
        <div className="empty-library-banner">
          <div className="empty-library-content">
            <h3>No Android Apps Installed Yet</h3>
            <p>
              Get started by clicking <strong>"Install APK"</strong> to sideload an APK file,
              or head over to <strong>"Install Tools"</strong> to set up Google Play Store.
            </p>
          </div>
        </div>
      )}

      {/* Custom Context Menu */}
      {contextMenu && (
        <ContextMenu
          position={contextMenu.position}
          app={contextMenu.app}
          onClose={handleCloseContextMenu}
          onLaunch={onLaunchApp}
          onCopyPackage={onCopyPackage}
          onUninstall={onUninstallApp}
        />
      )}
    </div>
  );
}
