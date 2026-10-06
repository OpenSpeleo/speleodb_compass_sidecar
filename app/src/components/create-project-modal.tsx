import { useState, type CSSProperties, type FormEvent } from "react";
import countriesData from "./assets/countries.json";
import { controller } from "../lib/controller";
import { compareUnicode } from "./project-status";
import { rustTrim } from "../lib/text";

// Load countries once and sort by their display names, rather than country codes.
const countries = Object.entries(countriesData).sort((a, b) =>
  compareUnicode(a[1], b[1]),
);
export interface CreateProjectFields {
  name: string;
  description: string;
  country: string;
  latitude: string;
  longitude: string;
}
// Rust f64::from_str consumes the entire string and permits its IEEE special values.
export function validCoordinate(value: string): boolean {
  return /^[+-]?(?:(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?|inf(?:inity)?|nan)(?![\s\S])/i.test(
    value,
  );
}
export function validateCreateProject(
  fields: CreateProjectFields,
): string | null {
  // Validate required fields first, then optional latitude/longitude values.
  if (rustTrim(fields.name) === "") return "Project name is required";
  if (new TextEncoder().encode(fields.name).length > 255)
    return "Project name must be less than 255 characters";
  if (rustTrim(fields.description) === "") return "Description is required";
  if (fields.country === "") return "Please select a country";
  if (fields.latitude !== "" && !validCoordinate(fields.latitude))
    return "Latitude must be a valid number";
  if (fields.longitude !== "" && !validCoordinate(fields.longitude))
    return "Longitude must be a valid number";
  return null;
}
const fieldStyle: CSSProperties = {
  width: "100%",
  padding: "8px",
  border: "1px solid transparent",
  borderRadius: "8px",
  boxSizing: "border-box",
  fontFamily: "inherit",
  fontSize: "14px",
  backgroundColor: "rgb(205, 205, 205)",
  color: "#313131",
};
const labelStyle: CSSProperties = {
  display: "block",
  marginBottom: "4px",
  fontWeight: 500,
  color: "#f6f6f6",
};

export function CreateProjectModal({ onClose }: { onClose: () => void }) {
  const [fields, setFields] = useState<CreateProjectFields>({
    name: "",
    description: "",
    country: "",
    latitude: "",
    longitude: "",
  });
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const update = (key: keyof CreateProjectFields, value: string) =>
    setFields((previous) => ({ ...previous, [key]: value }));
  async function submit(event: FormEvent) {
    event.preventDefault();
    const validation = validateCreateProject(fields);
    if (validation !== null) {
      setError(validation);
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await controller.createProject(
        fields.name,
        fields.description,
        fields.country,
        fields.latitude || null,
        fields.longitude || null,
      );
    } catch (error) {
      console.error("Error creating project:", error);
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  }
  return (
    <div
      className="modal"
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        width: "100vw",
        height: "100vh",
        backgroundColor: "rgba(0, 0, 0, 0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
    >
      <div
        className="modal-card"
        style={{
          backgroundColor: "rgba(41, 62, 112, 1)",
          color: "#f6f6f6",
          borderRadius: "12px",
          padding: "24px",
          maxWidth: "600px",
          width: "90%",
          boxShadow: "0 10px 25px rgba(0, 0, 0, 0.2)",
          maxHeight: "90vh",
          overflowY: "auto",
        }}
      >
        <h2 style={{ marginTop: 0, marginBottom: "20px", color: "#f6f6f6" }}>
          Create New Project
        </h2>
        {error !== null && (
          <div
            style={{
              padding: "12px",
              backgroundColor: "#fee2e2",
              border: "1px solid #ef4444",
              borderRadius: "6px",
              marginBottom: "16px",
              color: "#b91c1c",
            }}
          >
            {error}
          </div>
        )}
        <form
          onSubmit={(event) => {
            void submit(event);
          }}
        >
          <div style={{ marginBottom: "16px" }}>
            <label style={labelStyle}>Project Name *</label>
            <input
              type="text"
              value={fields.name}
              onChange={(event) => update("name", event.currentTarget.value)}
              style={fieldStyle}
              placeholder="My Awesome Cave Project"
              disabled={submitting}
            />
          </div>
          <div style={{ marginBottom: "16px" }}>
            <label style={labelStyle}>Description *</label>
            <textarea
              value={fields.description}
              onChange={(event) =>
                update("description", event.currentTarget.value)
              }
              style={{ ...fieldStyle, minHeight: "80px", resize: "vertical" }}
              placeholder="Describe the project..."
              disabled={submitting}
            />
          </div>
          <div style={{ marginBottom: "16px" }}>
            <label style={labelStyle}>Country *</label>
            <select
              value={fields.country}
              onChange={(event) => update("country", event.currentTarget.value)}
              style={fieldStyle}
              disabled={submitting}
            >
              <option value="" disabled>
                Select a country...
              </option>
              {countries.map(([code, name]) => (
                <option key={code} value={code}>
                  {name}
                </option>
              ))}
            </select>
          </div>
          <div style={{ marginBottom: "24px" }}>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "1fr 1fr",
                gap: "12px",
              }}
            >
              <div>
                <label style={labelStyle}>Latitude</label>
                <input
                  type="text"
                  value={fields.latitude}
                  onChange={(event) =>
                    update("latitude", event.currentTarget.value)
                  }
                  style={fieldStyle}
                  placeholder="e.g. 45.1234"
                  disabled={submitting}
                />
              </div>
              <div>
                <label style={labelStyle}>Longitude</label>
                <input
                  type="text"
                  value={fields.longitude}
                  onChange={(event) =>
                    update("longitude", event.currentTarget.value)
                  }
                  style={fieldStyle}
                  placeholder="e.g. -93.5678"
                  disabled={submitting}
                />
              </div>
            </div>
          </div>
          <div
            style={{ display: "flex", justifyContent: "flex-end", gap: "12px" }}
          >
            <button
              type="button"
              onClick={onClose}
              style={{
                padding: "8px 16px",
                border: "1px solid #d1d5db",
                borderRadius: "6px",
                backgroundColor: "white",
                color: "#374151",
                cursor: "pointer",
              }}
              disabled={submitting}
            >
              Cancel
            </button>
            <button
              type="submit"
              style={{
                padding: "8px 16px",
                border: "none",
                borderRadius: "6px",
                backgroundColor: "#2563eb",
                color: "white",
                cursor: "pointer",
                fontWeight: 500,
              }}
              disabled={submitting}
            >
              {submitting ? "Creating..." : "Create Project"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
