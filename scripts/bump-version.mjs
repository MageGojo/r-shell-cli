#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import readline from 'readline';

const colors = {
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  reset: '\x1b[0m',
};

const log = {
  info: (message) => console.log(`${colors.blue}${message}${colors.reset}`),
  success: (message) => console.log(`${colors.green}${message}${colors.reset}`),
  warn: (message) => console.log(`${colors.yellow}${message}${colors.reset}`),
  error: (message) => console.log(`${colors.red}${message}${colors.reset}`),
};

const args = process.argv.slice(2);
const bumpType = args[0] || 'patch';
const noCommit = args.includes('--no-commit');
const skipChangelog = args.includes('--skip-changelog');

if (!['major', 'minor', 'patch'].includes(bumpType)) {
  log.error(`Invalid bump type '${bumpType}'. Use major, minor, or patch.`);
  process.exit(1);
}

const rootDir = process.cwd();
const packageJsonPath = path.join(rootDir, 'package.json');
const cargoTomlPath = path.join(rootDir, 'cli', 'Cargo.toml');
const changelogPath = path.join(rootDir, 'CHANGELOG.md');

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
const currentVersion = packageJson.version;
const [major, minor, patch] = currentVersion.split('.').map(Number);

const newVersion = {
  major: `${major + 1}.0.0`,
  minor: `${major}.${minor + 1}.0`,
  patch: `${major}.${minor}.${patch + 1}`,
}[bumpType];

log.info(`Current version: ${currentVersion}`);
log.success(`New version: ${newVersion}`);

const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
});

rl.question(`Bump version from ${currentVersion} to ${newVersion}? (y/n) `, (answer) => {
  rl.close();
  if (answer.toLowerCase() !== 'y') {
    log.warn('Version bump cancelled');
    return;
  }

  try {
    packageJson.version = newVersion;
    fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);

    let cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
    cargoToml = cargoToml.replace(/^version = ".*"$/m, `version = "${newVersion}"`);
    fs.writeFileSync(cargoTomlPath, cargoToml);

    try {
      execSync('cargo check --quiet', {
        cwd: path.join(rootDir, 'cli'),
        stdio: 'ignore',
      });
    } catch {
      log.warn('Cargo check failed while updating lockfile; inspect manually before release.');
    }

    if (!skipChangelog && fs.existsSync(changelogPath)) {
      const today = new Date().toISOString().split('T')[0];
      const changelog = fs.readFileSync(changelogPath, 'utf8');
      const section = `\n## [${newVersion}] - ${today}\n\n### Added\n\n- _Add new features here_\n\n### Changed\n\n- _Add changes here_\n\n### Fixed\n\n- _Add bug fixes here_\n`;
      fs.writeFileSync(
        changelogPath,
        changelog.replace(/(## \[Unreleased\][^\n]*\n)/, `$1${section}\n`),
      );
      log.warn('Update CHANGELOG.md with real release notes before committing.');
    }

    if (!noCommit) {
      execSync('git add package.json cli/Cargo.toml cli/Cargo.lock');
      if (!skipChangelog && fs.existsSync(changelogPath)) {
        execSync('git add CHANGELOG.md');
      }
      execSync(`git commit -m "chore: bump version to ${newVersion}"`);
      log.success(`Version bumped to ${newVersion} and committed.`);
    } else {
      log.success(`Version bumped to ${newVersion}.`);
      log.warn('Files modified but not committed: package.json, cli/Cargo.toml, cli/Cargo.lock');
    }
  } catch (error) {
    log.error(`Version bump failed: ${error.message}`);
    process.exit(1);
  }
});
