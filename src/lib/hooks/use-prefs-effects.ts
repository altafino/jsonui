import { useEffect } from "react";
import { useApp } from "../store/app-store";

/** Rail breakpoint: below 1180 px the sidebar is a 56 px rail (DD12). */
export const RAIL_BREAKPOINT = 1180;

/** Applies theme, density and font scale to <html>, and tracks the rail breakpoint. */
export function usePrefsEffects() {
  const prefs = useApp((s) => s.prefs);
  const setRail = useApp((s) => s.setRail);

  useEffect(() => {
    const root = document.documentElement;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => root.setAttribute("data-theme", prefs.theme === "system" ? (mq.matches ? "dark" : "light") : prefs.theme);
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [prefs.theme]);

  useEffect(() => {
    document.documentElement.setAttribute("data-density", prefs.density);
    document.documentElement.style.setProperty("--font-scale", String(prefs.fontScale));
  }, [prefs.density, prefs.fontScale]);

  useEffect(() => {
    const mq = window.matchMedia(`(max-width: ${RAIL_BREAKPOINT - 1}px)`);
    const on = () => setRail(mq.matches);
    on();
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, [setRail]);
}
