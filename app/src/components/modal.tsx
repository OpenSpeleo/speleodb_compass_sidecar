export type ModalType = "Info" | "Warning" | "Success";

interface ModalProps {
  title: string;
  message: string;
  modalType?: ModalType;
  onClose?: () => void;
  primaryButtonText?: string;
  primaryButtonId?: string;
  onPrimaryAction?: () => void;
  showCloseButton?: boolean;
  closeButtonText?: string;
}

export function Modal({
  title,
  message,
  modalType = "Info",
  onClose,
  primaryButtonText,
  primaryButtonId,
  onPrimaryAction,
  showCloseButton = false,
  closeButtonText = "Close",
}: ModalProps) {
  const [icon, color] = {
    Info: ["ℹ️", "#3b82f6"],
    Warning: ["⚠️", "#f59e0b"],
    Success: ["✅", "#10b981"],
  }[modalType];
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
          backgroundColor: "white",
          borderRadius: "12px",
          padding: "24px",
          maxWidth: "500px",
          width: "90%",
          boxShadow: "0 10px 25px rgba(0, 0, 0, 0.2)",
          borderTop: `4px solid ${color}`,
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "12px",
            marginBottom: "16px",
          }}
        >
          <span style={{ fontSize: "32px" }}>{icon}</span>
          <h3 style={{ margin: 0, fontSize: "20px", color: "#1f2937" }}>
            {title}
          </h3>
        </div>
        <p
          style={{
            color: "#4b5563",
            lineHeight: 1.6,
            marginBottom: "20px",
            whiteSpace: "pre-line",
          }}
        >
          {message}
        </p>
        <div
          style={{ display: "flex", justifyContent: "flex-end", gap: "12px" }}
        >
          {showCloseButton && (
            <button
              onClick={onClose}
              style={{
                padding: "8px 16px",
                border: "1px solid #d1d5db",
                borderRadius: "6px",
                backgroundColor: "white",
                color: "#374151",
                cursor: "pointer",
                fontSize: "14px",
                transition: "background-color 0.2s",
              }}
            >
              {closeButtonText}
            </button>
          )}
          {primaryButtonText !== undefined && (
            <button
              id={primaryButtonId}
              onClick={onPrimaryAction}
              style={{
                padding: "8px 16px",
                border: "none",
                borderRadius: "6px",
                backgroundColor: color,
                color: "white",
                cursor: "pointer",
                fontSize: "14px",
                fontWeight: 500,
                transition: "opacity 0.2s",
              }}
            >
              {primaryButtonText}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
