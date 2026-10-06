import type { NextConfig } from "next";
import path from "node:path";

const config: NextConfig = {
  turbopack: { root: path.resolve(".") }, // a stray package-lock.json in the home folder otherwise confuses root detection
  // The Python backend (server.py) owns /api; the browser only ever talks to this origin.
  async rewrites() {
    return [{ source: "/api/:path*", destination: "http://127.0.0.1:8787/api/:path*" }];
  },
};
export default config;
