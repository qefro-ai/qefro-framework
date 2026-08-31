import { FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api, saveToken } from "../api";

export default function Login() {
  const [mode, setMode] = useState<"login" | "register">("login");
  const [error, setError] = useState("");
  const navigate = useNavigate();

  async function onSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError("");
    const data = new FormData(e.currentTarget);
    try {
      if (mode === "login") {
        const res = await api.login(String(data.get("email")), String(data.get("password")));
        saveToken(res.access_token, res.expires_in);
      } else {
        const res = await api.register({
          name: String(data.get("name")),
          email: String(data.get("email")),
          password: String(data.get("password")),
          tenant_name: String(data.get("tenant_name")),
          tenant_slug: String(data.get("tenant_slug")),
        });
        saveToken(res.access_token, res.expires_in);
      }
      navigate("/", { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed");
    }
  }

  return (
    <div className="login-screen">
      <div className="login">
        <p className="badge">Workspace</p>
        <h1>{mode === "login" ? "Welcome back" : "Create a workspace"}</h1>
        <p className="muted">
          {mode === "login"
            ? "Sign in to open your tenant’s application."
            : "Register a tenant. Branding and apps can be configured after you sign in."}
        </p>
        <form className="form" onSubmit={onSubmit}>
          {mode === "register" && (
            <>
              <label>Your name<input name="name" required /></label>
              <label>Company / tenant name<input name="tenant_name" required /></label>
              <label>Tenant slug<input name="tenant_slug" required placeholder="acme" /></label>
            </>
          )}
          <label>Email<input name="email" type="email" required autoComplete="email" /></label>
          <label>
            Password
            <input name="password" type="password" required minLength={8} autoComplete="current-password" />
          </label>
          {error && <p className="error">{error}</p>}
          <button type="submit">{mode === "login" ? "Sign in" : "Create tenant"}</button>
          <button
            type="button"
            className="ghost"
            onClick={() => setMode(mode === "login" ? "register" : "login")}
          >
            {mode === "login" ? "Need an account?" : "Have an account?"}
          </button>
        </form>
      </div>
    </div>
  );
}
