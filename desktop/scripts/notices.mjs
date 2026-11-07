import { createRequire } from "node:module";
import {
  readFileSync,
  readdirSync,
  writeFileSync,
  mkdirSync,
  existsSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const desktop = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = resolve(desktop, "../artifacts/build/tauri-notices");
mkdirSync(output, { recursive: true });
const own = readFileSync(join(desktop, "THIRD_PARTY_NOTICES.md"), "utf8");
const visited = new Set();
const sections = [own];
function collect(name, from) {
  const require = createRequire(from);
  let manifest;
  try {
    manifest = require.resolve(`${name}/package.json`);
  } catch {
    let path = dirname(require.resolve(name));
    while (!existsSync(join(path, "package.json"))) {
      const parent = dirname(path);
      if (parent === path) throw new Error(`未找到依赖清单：${name}`);
      path = parent;
    }
    manifest = join(path, "package.json");
  }
  const pkg = JSON.parse(readFileSync(manifest, "utf8"));
  const key = `${pkg.name}@${pkg.version}`;
  if (visited.has(key)) return;
  visited.add(key);
  const directory = dirname(manifest);
  const licenses = readdirSync(directory, { withFileTypes: true }).filter(
    (e) =>
      e.isFile() && /^(license|licence|copying|notice)([.\-]|$)/i.test(e.name),
  );
  sections.push(
    `\n\n## ${key}\nLicense: ${pkg.license || "见项目许可证"}\n${licenses.map((e) => readFileSync(join(directory, e.name), "utf8")).join("\n")}`,
  );
  for (const dep of Object.keys(pkg.dependencies || {})) collect(dep, manifest);
}
const root = join(desktop, "package.json");
for (const dependency of Object.keys(
  JSON.parse(readFileSync(root, "utf8")).dependencies,
))
  collect(dependency, root);
writeFileSync(
  join(output, "FRONTEND-LICENSES.txt"),
  sections.join("\n"),
  "utf8",
);
if (process.argv.includes("--rust")) {
  const args = [
    "metadata",
    "--format-version",
    "1",
    "--locked",
    "--offline",
    "--filter-platform",
    "x86_64-pc-windows-msvc",
  ];
  const metadata = JSON.parse(
    execFileSync("cargo", args, {
      cwd: resolve(desktop, ".."),
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
    }),
  );
  const tree = execFileSync(
    "cargo",
    [
      "tree",
      "-p",
      "githubsp-desktop",
      "--target",
      "x86_64-pc-windows-msvc",
      "--locked",
      "--offline",
      "-e",
      "normal,no-proc-macro",
      "--prefix",
      "none",
      "--format",
      "{p}",
    ],
    { cwd: resolve(desktop, ".."), encoding: "utf8" },
  );
  const crates = new Set(
    tree
      .split(/\r?\n/)
      .map((line) =>
        line
          .match(/^([^ ]+) v([^ ]+)/)
          ?.slice(1)
          .join("@"),
      )
      .filter(Boolean),
  );
  const rust = ["GitHubSP Rust 运行依赖声明"];
  for (const pkg of metadata.packages.filter((pkg) =>
    crates.has(`${pkg.name}@${pkg.version}`),
  )) {
    const directory = dirname(pkg.manifest_path);
    const files = readdirSync(directory, { withFileTypes: true }).filter(
      (e) =>
        e.isFile() &&
        /^(license|licence|copying|notice)([.\-]|$)/i.test(e.name),
    );
    rust.push(
      `\n## ${pkg.name}@${pkg.version}\nLicense: ${pkg.license || "见源仓库"}\nSource: ${pkg.repository || pkg.source || "GitHubSP workspace"}\n${files.map((e) => readFileSync(join(directory, e.name), "utf8")).join("\n")}`,
    );
  }
  writeFileSync(join(output, "RUST-LICENSES.txt"), rust.join("\n"), "utf8");
}
console.log(
  `版权声明已生成：${visited.size} 个前端依赖${process.argv.includes("--rust") ? "及 Rust 运行依赖" : ""}。`,
);
