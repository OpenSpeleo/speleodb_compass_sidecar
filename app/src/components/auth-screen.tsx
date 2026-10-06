import { useState, type FormEvent } from "react";
import { controller, validateOauth as validOAuth } from "../lib/controller";
import { API_BASE_URL } from "../lib/constants";

export const AUTH_METHOD_ERROR =
  "Must provide exactly one auth method: either email+password or a 40-char OAUTH token";
export { validOAuth };
export function validEmailField(value: string): boolean {
  if (value === "") return true;
  const parts = value.split("@");
  return (
    parts.length === 2 &&
    parts[0] !== "" &&
    parts[1]!.includes(".") &&
    new TextEncoder().encode(parts[1]).length > 2
  );
}
export function isCredentialsError(message: string): boolean {
  // Credentials failures include the existing 400/401/403 and forbidden messages.
  return (
    message.includes("Invalid credentials") ||
    ["403", "401", "400"].some((code) => message.includes(code)) ||
    message.toLowerCase().includes("forbidden")
  );
}

export function AuthScreen() {
  // Fields
  const [instance, setInstance] = useState(API_BASE_URL);
  // Presence is intentionally distinct from an empty, previously edited field.
  const [email, setEmail] = useState<string | null>(null);
  const [password, setPassword] = useState<string | null>(null);
  const [oauth, setOAuth] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [showError, setShowError] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const [credentialError, setCredentialError] = useState(false);
  // Validation is silent at startup/auto-login and becomes visible on interaction.
  const [validationSilent, setValidationSilent] = useState(true);

  async function connect(event: FormEvent) {
    event.preventDefault();
    setValidationSilent(false);
    // Quick validation uses the submitted render's field snapshot throughout the request.
    const oauthOK = oauth !== null && validOAuth(oauth);
    const parts = email?.split("@");
    const passOK =
      parts !== undefined &&
      password !== null &&
      parts.length === 2 &&
      parts[1]!.includes(".");
    if (oauthOK === passOK) {
      setErrorMessage(AUTH_METHOD_ERROR);
      setCredentialError(false);
      setShowError(true);
      return;
    }
    setLoading(true);
    let url: string;
    try {
      url = new URL(instance).href;
    } catch {
      setErrorMessage("Instance URL is invalid");
      setCredentialError(false);
      setShowError(true);
      setLoading(false);
      return;
    }
    try {
      await controller.authenticate(email, password, oauth, url);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setCredentialError(isCredentialsError(message));
      setErrorMessage(message);
      setShowError(true);
    } finally {
      setLoading(false);
    }
  }
  // Form handlers
  function reset() {
    setInstance("https://www.speleodb.org");
    setEmail(null);
    setPassword(null);
    setOAuth(null);
    setShowError(false);
    setErrorMessage("");
  }
  // Derived UI state: flag mutually exclusive methods and incomplete credentials.
  // Keep Connect enabled for invalid fields so submission can explain validation errors.
  const hasEmail = email !== null,
    hasPassword = password !== null,
    hasOAuth = oauth !== null;
  const both = hasOAuth && (hasEmail || hasPassword);
  const emailInvalid =
    !validationSilent && email !== null && !validEmailField(email);
  const oauthInvalid =
    !validationSilent && oauth !== null && oauth !== "" && !validOAuth(oauth);
  const emailConflict = both && hasEmail,
    passwordConflict = both && hasPassword,
    oauthConflict = both && hasOAuth;
  const missingPassword = hasEmail && !hasPassword && !hasOAuth;
  const missingEmail = hasPassword && !hasEmail && !hasOAuth;
  const actionStyle = {
    color: "white",
    border: "none",
    padding: "8px 16px",
    borderRadius: "4px",
    cursor: "pointer",
    fontWeight: 500,
    width: "14em",
  };
  return (
    <main className="container">
      <h1>SpeleoDB - Compass Sidecar</h1>
      <div className="row">
        <img
          src="public/speleodb_long.png"
          className="logo speleodb"
          alt="SpeleoDB logo"
        />
      </div>
      <form
        onSubmit={(event) => {
          void connect(event);
        }}
        className="auth-form"
      >
        <div className="auth-group">
          <label htmlFor="instance">SpeleoDB instance</label>
          {/* Auto-trim trailing slashes on blur; the existing instance input stays uncontrolled. */}
          <input
            id="instance"
            type="text"
            className="full"
            onInput={(event) => {
              setInstance(event.currentTarget.value);
              setValidationSilent(false);
            }}
            onBlur={(event) => {
              if (event.currentTarget.value.endsWith("/"))
                setInstance(event.currentTarget.value.replace(/\/+$/, ""));
            }}
            placeholder={API_BASE_URL}
          />
        </div>
        <div className="accent-bar" aria-hidden="true" />
        <p className="hint">
          Authenticate with either Email &amp; Password or OAuth token.
        </p>
        <div className="auth-group">
          <label htmlFor="email">Email</label>
          <input
            id="email"
            type="email"
            className={
              emailInvalid || emailConflict || missingPassword
                ? "full invalid"
                : "full"
            }
            value={email ?? ""}
            onChange={(event) => {
              setEmail(event.currentTarget.value);
              setValidationSilent(false);
            }}
            placeholder="your@email.com"
          />
          {emailInvalid ? (
            <span className="field-error">Invalid email format</span>
          ) : emailConflict ? (
            <span className="field-error">
              Cannot use both email/password AND OAuth token
            </span>
          ) : missingPassword ? (
            <span className="field-error">
              Password is required when using email
            </span>
          ) : null}
        </div>
        <div className="auth-group">
          <label htmlFor="password">Password</label>
          <input
            id="password"
            type="password"
            placeholder="Your SpeleoDB Password"
            className={
              passwordConflict || missingEmail ? "full invalid" : "full"
            }
            value={password ?? ""}
            onChange={(event) => {
              setPassword(event.currentTarget.value);
              setValidationSilent(false);
            }}
          />
          {passwordConflict ? (
            <span className="field-error">
              Cannot use both email/password AND OAuth token
            </span>
          ) : missingEmail ? (
            <span className="field-error">
              Email is required when using password
            </span>
          ) : null}
        </div>
        <hr
          style={{
            borderTop: "dotted 1px #000",
            backgroundColor: "transparent",
            borderStyle: "none none dotted",
            width: "100%",
          }}
        />
        <div className="auth-group">
          <label htmlFor="oauth">OAUTH Token</label>
          <input
            id="oauth"
            type="text"
            className={oauthInvalid || oauthConflict ? "full invalid" : "full"}
            value={oauth ?? ""}
            onChange={(event) => {
              setOAuth(event.currentTarget.value);
              setValidationSilent(false);
            }}
            placeholder="Your SpeleoDB OAuth token"
          />
          {oauthInvalid ? (
            <span className="field-error">
              Must be 40 hexadecimal characters
            </span>
          ) : oauthConflict ? (
            <span className="field-error">
              Cannot use both OAuth token AND email/password
            </span>
          ) : null}
        </div>
        <div className="accent-bar" aria-hidden="true" />
        <div className="actions">
          <button
            type="button"
            onClick={reset}
            style={{ ...actionStyle, backgroundColor: "#6b7280" }}
          >
            Reset Form
          </button>
          <button
            type="submit"
            disabled={loading}
            style={{ ...actionStyle, backgroundColor: "#2563eb" }}
          >
            {loading ? "Connecting..." : "Connect"}
          </button>
        </div>
      </form>
      {showError && (
        <div className="modal">
          <div className="modal-card">
            <h3>Connection failed</h3>
            {credentialError ? (
              <>
                <p className="error-403">
                  <strong>Invalid credentials</strong>
                </p>
                <p>
                  The email/password or OAuth token you provided is incorrect.
                  Please check your credentials and try again.
                </p>
              </>
            ) : (
              <p>{errorMessage}</p>
            )}
            <div
              style={{
                display: "flex",
                justifyContent: "flex-end",
                gap: "8px",
                marginTop: "12px",
              }}
            >
              <button
                onClick={() => {
                  setShowError(false);
                  setCredentialError(false);
                }}
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}
