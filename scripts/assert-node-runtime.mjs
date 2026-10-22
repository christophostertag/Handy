if (
  process.versions.bun ||
  Number(process.versions.node.split(".")[0]) !== 24
) {
  throw new Error(
    `Handy tooling requires Node 24.x; received ${process.release.name} ${process.version}`,
  );
}
