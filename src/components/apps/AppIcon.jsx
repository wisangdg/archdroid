import React, { useState } from "react";

/** First letters of the first two words, for apps without a usable icon. */
export function appInitials(app) {
  const words = (app.name || app.package || "?").trim().split(/\s+/);
  return words
    .slice(0, 2)
    .map((word) => word[0])
    .join("")
    .toUpperCase();
}

/**
 * The app's launcher icon, or its initials when there is none or it fails to
 * load. Decorative: the row or heading next to it carries the app's name.
 * Give it `key={app.package}` so a failed icon does not stick to the next app.
 */
export default function AppIcon({ app, size = 40, className = "" }) {
  const [failed, setFailed] = useState(false);
  return (
    <span
      className={`app-icon ${className}`}
      style={{ width: size, height: size, fontSize: Math.round(size * 0.36) }}
      aria-hidden="true"
    >
      {app.icon && !failed ? (
        <img
          src={app.icon}
          alt=""
          className="app-icon-img"
          loading="lazy"
          onError={() => setFailed(true)}
        />
      ) : (
        <span className="app-icon-initials">{appInitials(app)}</span>
      )}
    </span>
  );
}
