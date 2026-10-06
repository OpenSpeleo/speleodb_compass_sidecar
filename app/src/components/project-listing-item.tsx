import { useState } from "react";
import type { ProjectStatus } from "../lib/types";
import { controller } from "../lib/controller";
import {
  COLOR_ALARM,
  COLOR_BLUE,
  COLOR_GOOD,
  COLOR_GREY,
  COLOR_WARN,
  FONT_COLOR_BLUE,
} from "../lib/constants";
import { Icon } from "../icons";
import { projectStatusPresentation } from "./project-status";

export function ProjectListingItem({
  project,
  userEmail,
}: {
  project: ProjectStatus;
  userEmail: string;
}) {
  const [navigationError, setNavigationError] = useState<string | null>(null);
  const { info } = project;
  const permissionColor =
    info.permission === "ADMIN"
      ? COLOR_WARN
      : info.permission === "READ_AND_WRITE"
        ? COLOR_BLUE
        : COLOR_GREY;
  const lockColor =
    info.active_mutex === null
      ? COLOR_GOOD
      : info.active_mutex.user === userEmail
        ? COLOR_WARN
        : COLOR_ALARM;
  const lockStatus =
    info.active_mutex === null
      ? "🔓 editable"
      : info.active_mutex.user === userEmail
        ? "🔒 by me"
        : `🔒 by ${info.active_mutex.user}`;
  const status = projectStatusPresentation(project.local_status);
  const badge = {
    padding: "4px 8px",
    borderRadius: "4px",
    color: "white",
    fontSize: "12px",
    fontWeight: "bold",
  };
  async function open() {
    setNavigationError(null);
    try {
      await controller.setActiveProject(info.id);
    } catch (error) {
      setNavigationError(
        `Could not open this project: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }
  return (
    <>
      <div
        className="project-card"
        onClick={() => {
          void open();
        }}
      >
        <span
          style={{
            padding: "4px 8px",
            borderRadius: "4px",
            fontSize: "12px",
            display: "flex",
            gap: "12px",
            color: status.color,
          }}
        >
          <Icon name={status.icon} />
          <h3
            className="vertically-centered-text"
            style={{ margin: 0, fontSize: "16px", color: FONT_COLOR_BLUE }}
          >
            {Array.from(info.name).slice(0, 25).join("")}
          </h3>
        </span>
        <div style={{ display: "flex", gap: "12px" }}>
          <span style={{ ...badge, backgroundColor: permissionColor }}>
            {info.permission}
          </span>
          <span style={{ ...badge, backgroundColor: lockColor }}>
            {lockStatus}
          </span>
        </div>
      </div>
      {navigationError !== null && (
        <p
          role="alert"
          style={{
            margin: 0,
            color: "#fecaca",
            fontSize: "13px",
            textAlign: "left",
          }}
        >
          {navigationError}
        </p>
      )}
    </>
  );
}
