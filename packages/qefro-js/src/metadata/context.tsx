import { createContext, useContext } from "react";
import type { TenantTheme } from "./types";

export const TenantThemeContext = createContext<TenantTheme>({
  timezone: "UTC",
  locale: "en-US",
  currency: "USD",
});

export function useTenantTheme() {
  return useContext(TenantThemeContext);
}
