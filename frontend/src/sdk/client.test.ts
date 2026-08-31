import { api, clearToken, hasToken, saveToken, TOKEN_KEY } from "./client";

describe("browser token storage", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    clearToken();
    localStorage.clear();
  });

  it("stores the token only in localStorage, never in the URL", () => {
    saveToken("access-secret-token", 3600);
    expect(localStorage.getItem(TOKEN_KEY)).toBe("access-secret-token");
    expect(hasToken()).toBe(true);
    expect(window.location.href).not.toContain("access-secret-token");
    expect(window.location.search).not.toContain("token");
  });

  it("clears the token on logout", () => {
    saveToken("access-secret-token", 3600);
    clearToken();
    expect(localStorage.getItem(TOKEN_KEY)).toBeNull();
    expect(localStorage.getItem("qefro_token_exp")).toBeNull();
    expect(hasToken()).toBe(false);
  });

  it("replaces the stored token on tenant switch", async () => {
    saveToken("old-tenant-token", 3600);
    vi.spyOn(globalThis, "fetch").mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ access_token: "new-tenant-token", expires_in: 3600 }),
    } as Response);
    await api.switchTenant("11111111-1111-1111-1111-111111111111");
    expect(localStorage.getItem(TOKEN_KEY)).toBe("new-tenant-token");
    expect(localStorage.getItem(TOKEN_KEY)).not.toBe("old-tenant-token");
  });
});
