import type { GlobalThemeOverrides } from "naive-ui";

// Keep Naive UI's primary color aligned with the app accent. The dark palette
// uses a slightly lighter accent so it stays readable on dark surfaces.
const common: GlobalThemeOverrides["common"] = {
  fontFamily:
    'Inter, Avenir, "Microsoft YaHei", "PingFang SC", "Noto Sans SC", Helvetica, Arial, sans-serif',
  borderRadius: "8px",
};

export const lightThemeOverrides: GlobalThemeOverrides = {
  common: {
    ...common,
    primaryColor: "#396cd8",
    primaryColorHover: "#535bf2",
    primaryColorPressed: "#2f57b8",
    primaryColorSuppl: "#396cd8",
  },
};

export const darkThemeOverrides: GlobalThemeOverrides = {
  common: {
    ...common,
    primaryColor: "#5c8dff",
    primaryColorHover: "#7ba3ff",
    primaryColorPressed: "#4673d6",
    primaryColorSuppl: "#5c8dff",
  },
};
