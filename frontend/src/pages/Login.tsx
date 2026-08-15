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
        saveToken(res.access_token);
      } else {
        const res = await api.register({
          name: String(data.get("name")),
          email: String(data.get("email")),
          password: String(data.get("password")),
          tenant_name: String(data.get("tenant_name")),
          tenant_slug: String(data.get("tenant_slug")),
        });
        saveToken(res.access_token);
      }
      navigate("/");
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed");
    }
  }

  return (
    <div className="login">
      <h1>Qefro</h1>
      <p className="muted">Sign in to a tenant. UI is generated from entity metadata.</p>
      <form className="form" onSubmit={onSubmit}>
        {mode === "register" && (
          <>
            <label>Name<input name="name" required /></label>
            <label>Tenant name<input name="tenant_name" required /></label>
            <label>Tenant slug<input name="tenant_slug" required /></label>
          </>
        )}
        <label>Email<input name="email" type="email" required /></label>
        <label>Password<input name="password" type="password" required minLength={8} /></label>
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
  );
}
