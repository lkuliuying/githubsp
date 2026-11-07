import type { Config } from "tailwindcss";
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        border: "var(--border)",
        input: "var(--border)",
        ring: "var(--accent)",
        background: "var(--surface)",
        foreground: "var(--ink)",
        primary: { DEFAULT: "var(--accent)", foreground: "#ffffff" },
        secondary: {
          DEFAULT: "var(--surface-muted)",
          foreground: "var(--ink)",
        },
        muted: { DEFAULT: "var(--surface-muted)", foreground: "var(--muted)" },
        accent: { DEFAULT: "var(--surface-hover)", foreground: "var(--ink)" },
        destructive: { DEFAULT: "var(--danger)", foreground: "#ffffff" },
      },
      borderRadius: { lg: "10px", md: "7px", sm: "5px" },
    },
  },
  plugins: [],
} satisfies Config;
