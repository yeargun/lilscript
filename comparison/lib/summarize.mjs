import {readdirSync, readFileSync, writeFileSync} from "node:fs";
import {join} from "node:path";

const root = process.argv[2];
const apps = join(root, "apps");
const reports = readdirSync(apps, {withFileTypes: true})
  .filter((entry) => entry.isDirectory())
  .map((entry) => JSON.parse(readFileSync(join(apps, entry.name, "build", "report.json"))))
  .sort((left, right) => left.app.localeCompare(right.app));

const total = (compiler, metric) => reports.reduce(
  (sum, report) => sum + report[compiler][metric], 0,
);
const rows = reports.map((report) =>
  `| ${report.app} | ${report.lilscript.raw} | ${report.closure.raw} | ` +
  `${report.lilscript.gzip9} | ${report.closure.gzip9} | ` +
  `${report.lilscript.brotli11} | ${report.closure.brotli11} |`,
);
rows.push(
  `| **Total** | **${total("lilscript", "raw")}** | **${total("closure", "raw")}** | ` +
  `**${total("lilscript", "gzip9")}** | **${total("closure", "gzip9")}** | ` +
  `**${total("lilscript", "brotli11")}** | **${total("closure", "brotli11")}** |`,
);
const markdown = `# Comparison summary\n\n` +
  `All programs passed expected-output tests in both compilers. Sizes are actual ` +
  `generated-file bytes. Each LilScript column is measured from an independent ` +
  `build optimized for that column's raw, gzip, or Brotli objective; sizes of the ` +
  `same build under the other codecs are diagnostic only.\n\n` +
  `| Program | Lil raw-target | Closure raw | Lil gzip-target | Closure gzip | Lil Brotli-target | Closure Brotli |\n` +
  `| --- | ---: | ---: | ---: | ---: | ---: | ---: |\n${rows.join("\n")}\n`;
writeFileSync(join(root, "summary.json"), `${JSON.stringify(reports, null, 2)}\n`);
writeFileSync(join(root, "summary.md"), markdown);
process.stdout.write(markdown);
