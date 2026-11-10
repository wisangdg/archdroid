import React, { useState, useMemo, useRef, useEffect } from "react";
import { Search, X, PackagePlus, AlertCircle } from "lucide-react";
import AppRow from "./AppRow";
import AppDetail from "./AppDetail";
import ContextMenu from "./ContextMenu";

const SYSTEM_PREFIXES = [
  "com.android.",
  "com.google.android.",
  "org.lineageos.",
  "com.google.android.inputmethod",
];

export function isSystemApp(pkg) {
  return SYSTEM_PREFIXES.some((prefix) => pkg.startsWith(prefix));
}

export const INITIAL_VIEW_STATE = {
  searchQuery: "",
  categoryFilter: "all",
  selectedPackage: null,
};

const FILTERS = [
  { id: "all", label: "All" },
  { id: "user", label: "User" },
  { id: "system", label: "System" },
];

/**
 * "Control Center" app library: a dense list on the left, the selected app
 * with all of its actions on the right.
 */
export default function MyAppsView({
  apps,
  engineOnline = false,
  booting = false,
  onLaunchApp,
  onForceStopApp,
  onOpenKeymap,
  onInstallApk,
  onUninstallApp,
  onCopyPackage,
  onBackupApk,
  savingApk = false,
  busy = false,
  viewState,
  onViewStateChange,
}) {
  // The page unmounts when another tab is opened. The app keeps this state
  // (`viewState`), so the search, the filter and the selection are still
  // there on the way back; standalone, the component holds it itself.
  const [ownState, setOwnState] = useState(INITIAL_VIEW_STATE);
  const view = viewState ?? ownState;
  const setView = onViewStateChange ?? setOwnState;
  const { searchQuery, categoryFilter, selectedPackage } = view;
  const setSearchQuery = (value) =>
    setView((prev) => ({ ...prev, searchQuery: value }));
  const setCategoryFilter = (value) =>
    setView((prev) => ({ ...prev, categoryFilter: value }));
  const setSelectedPackage = (value) =>
    setView((prev) => ({ ...prev, selectedPackage: value }));
  const [contextMenu, setContextMenu] = useState(null); // { position, app }
  const searchInputRef = useRef(null);
  const listRef = useRef(null);

  // Ctrl+F focuses the search field.
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
    const system = apps.filter((app) => isSystemApp(app.package)).length;
    return { all: apps.length, user: apps.length - system, system };
  }, [apps]);

  const filteredApps = useMemo(() => {
    let list = apps;
    if (categoryFilter === "user") {
      list = list.filter((app) => !isSystemApp(app.package));
    } else if (categoryFilter === "system") {
      list = list.filter((app) => isSystemApp(app.package));
    }
    const query = searchQuery.toLowerCase().trim();
    if (!query) return list;
    return list.filter(
      (app) =>
        app.name.toLowerCase().includes(query) ||
        app.package.toLowerCase().includes(query)
    );
  }, [apps, categoryFilter, searchQuery]);

  // The selection follows the visible list: if the selected app is filtered
  // out or uninstalled, the first visible app is shown instead.
  const selectedApp =
    filteredApps.find((app) => app.package === selectedPackage) ??
    filteredApps[0] ??
    null;

  const moveSelection = (delta) => {
    if (!selectedApp) return;
    const index = filteredApps.indexOf(selectedApp);
    const next = filteredApps[index + delta];
    if (!next) return;
    setSelectedPackage(next.package);
    const row = [...(listRef.current?.querySelectorAll(".app-row") ?? [])].find(
      (el) => el.dataset.package === next.package
    );
    row?.focus();
  };

  const count = `${apps.length} application${apps.length === 1 ? "" : "s"}`;
  const subtitle = engineOnline
    ? `${count} ready to launch`
    : `${count}. Launching one starts the engine.`;

  return (
    <div className="control-center">
      <section className="app-list-pane" aria-labelledby="app-list-title">
        <div className="app-list-head">
          <div className="app-list-titles">
            <h2 id="app-list-title">Applications</h2>
            <p className="app-list-subtitle">{subtitle}</p>
          </div>
          <button
            type="button"
            className="btn btn-primary btn-sm"
            onClick={onInstallApk}
            disabled={busy}
            title="Install an APK from your computer, or drop one on the window"
          >
            <PackagePlus size={16} aria-hidden="true" />
            <span>Install APK</span>
          </button>
        </div>

        <div className="search-box">
          <Search size={16} className="search-icon" aria-hidden="true" />
          <label htmlFor="app-search" className="sr-only">
            Search apps or packages
          </label>
          <input
            id="app-search"
            ref={searchInputRef}
            type="text"
            className="search-input"
            placeholder="Search name or package (Ctrl + F)"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
          {searchQuery && (
            <button
              type="button"
              className="search-clear-btn"
              onClick={() => setSearchQuery("")}
              aria-label="Clear search"
              title="Clear search"
            >
              <X size={15} aria-hidden="true" />
            </button>
          )}
        </div>

        <div className="segmented" role="group" aria-label="Filter apps">
          {FILTERS.map((filter) => (
            <button
              key={filter.id}
              type="button"
              className={`segmented-btn ${categoryFilter === filter.id ? "active" : ""}`}
              aria-pressed={categoryFilter === filter.id}
              onClick={() => setCategoryFilter(filter.id)}
            >
              {filter.label}
              <span className="segmented-count">{counts[filter.id]}</span>
            </button>
          ))}
        </div>

        {apps.length === 0 ? (
          <div className="empty-library-banner">
            <h3>No Android apps installed yet</h3>
            <p>
              Click <strong>Install APK</strong> or drop an <code>.apk</code> or
              split APK bundle on the window. To get Google Play, open{" "}
              <strong>Install Tools</strong>.
            </p>
          </div>
        ) : filteredApps.length === 0 ? (
          <div className="empty-search-state">
            <AlertCircle size={30} className="empty-icon" aria-hidden="true" />
            <h3>No applications found</h3>
            <p>Nothing matches “{searchQuery}” in this filter.</p>
            <div className="empty-search-actions">
              {searchQuery && (
                <button
                  type="button"
                  className="btn btn-outline btn-sm"
                  onClick={() => setSearchQuery("")}
                >
                  Clear Search
                </button>
              )}
              {categoryFilter !== "all" && (
                <button
                  type="button"
                  className="btn btn-outline btn-sm"
                  onClick={() => setCategoryFilter("all")}
                >
                  Show All Apps
                </button>
              )}
            </div>
          </div>
        ) : (
          <ul className="app-list" ref={listRef} aria-label="Installed apps">
            {filteredApps.map((app) => (
              <li key={app.package || app.name}>
                <AppRow
                  app={app}
                  system={isSystemApp(app.package)}
                  selected={app === selectedApp}
                  onSelect={setSelectedPackage}
                  onLaunch={onLaunchApp}
                  onMove={moveSelection}
                  onContextMenu={(e, target) =>
                    setContextMenu({
                      position: { x: e.clientX, y: e.clientY },
                      app: target,
                    })
                  }
                  disabled={busy}
                />
              </li>
            ))}
          </ul>
        )}
      </section>

      <AppDetail
        app={selectedApp}
        system={selectedApp ? isSystemApp(selectedApp.package) : false}
        engineOnline={engineOnline}
        booting={booting}
        busy={busy}
        onLaunch={onLaunchApp}
        onForceStop={onForceStopApp}
        onOpenKeymap={onOpenKeymap}
        onCopyPackage={onCopyPackage}
        onBackupApk={onBackupApk}
        savingApk={savingApk}
        onUninstall={onUninstallApp}
      />

      {contextMenu && (
        <ContextMenu
          position={contextMenu.position}
          app={contextMenu.app}
          onClose={() => setContextMenu(null)}
          onLaunch={onLaunchApp}
          onForceStop={engineOnline ? onForceStopApp : undefined}
          onBackupApk={engineOnline && !booting ? onBackupApk : undefined}
          onCopyPackage={onCopyPackage}
          onUninstall={onUninstallApp}
          disabled={busy}
        />
      )}
    </div>
  );
}
