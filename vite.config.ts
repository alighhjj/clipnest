import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 期望一个固定端口的开发服务器，且不要清屏（否则会覆盖 Rust 日志）
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: false,
    watch: {
      // src-tauri 由 cargo 自己监听，避免 vite 重复触发
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    // Tauri 使用较新的 WebView2 / WKWebView / WebKitGTK
    target: "es2021",
    minify: "esbuild",
    sourcemap: false,
    chunkSizeWarningLimit: 1200,
  },
});
