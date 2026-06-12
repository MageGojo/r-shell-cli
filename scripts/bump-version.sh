#!/bin/bash

# R-Shell CLI version bump script.
# Usage: ./scripts/bump-version.sh [major|minor|patch] [--no-commit] [--skip-changelog]

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

BUMP_TYPE="${1:-patch}"
NO_COMMIT=false
SKIP_CHANGELOG=false

for arg in "$@"; do
  case $arg in
    --no-commit)
      NO_COMMIT=true
      ;;
    --skip-changelog)
      SKIP_CHANGELOG=true
      ;;
  esac
done

if [[ ! "$BUMP_TYPE" =~ ^(major|minor|patch)$ ]]; then
  echo -e "${RED}Invalid bump type '$BUMP_TYPE'. Use major, minor, or patch.${NC}"
  exit 1
fi

CURRENT_VERSION=$(node -p "require('./package.json').version")
IFS='.' read -r -a VERSION_PARTS <<< "$CURRENT_VERSION"
MAJOR="${VERSION_PARTS[0]}"
MINOR="${VERSION_PARTS[1]}"
PATCH="${VERSION_PARTS[2]}"

case $BUMP_TYPE in
  major)
    MAJOR=$((MAJOR + 1))
    MINOR=0
    PATCH=0
    ;;
  minor)
    MINOR=$((MINOR + 1))
    PATCH=0
    ;;
  patch)
    PATCH=$((PATCH + 1))
    ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
echo -e "${BLUE}Current version: ${CURRENT_VERSION}${NC}"
echo -e "${GREEN}New version: ${NEW_VERSION}${NC}"

read -p "Bump version from ${CURRENT_VERSION} to ${NEW_VERSION}? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  echo -e "${YELLOW}Version bump cancelled${NC}"
  exit 0
fi

node -e "const fs=require('fs'); const p='package.json'; const pkg=JSON.parse(fs.readFileSync(p,'utf8')); pkg.version='${NEW_VERSION}'; fs.writeFileSync(p, JSON.stringify(pkg,null,2)+'\n');"

if [[ "$OSTYPE" == "darwin"* ]]; then
  sed -i '' "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" cli/Cargo.toml
else
  sed -i "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" cli/Cargo.toml
fi

(cd cli && cargo check --quiet) || echo -e "${YELLOW}Cargo check failed while updating lockfile; inspect manually before release.${NC}"

if [ "$SKIP_CHANGELOG" = false ] && [ -f CHANGELOG.md ]; then
  CURRENT_DATE=$(date +%Y-%m-%d)
  TEMP_FILE=$(mktemp)
  awk -v version="$NEW_VERSION" -v date="$CURRENT_DATE" '
    /^## \[Unreleased\]/ {
      print
      print ""
      print "## [" version "] - " date
      print ""
      print "### Added"
      print ""
      print "- _Add new features here_"
      print ""
      print "### Changed"
      print ""
      print "- _Add changes here_"
      print ""
      print "### Fixed"
      print ""
      print "- _Add bug fixes here_"
      next
    }
    { print }
  ' CHANGELOG.md > "$TEMP_FILE"
  mv "$TEMP_FILE" CHANGELOG.md
  echo -e "${YELLOW}Update CHANGELOG.md with real release notes before committing.${NC}"
fi

if [ "$NO_COMMIT" = false ]; then
  git add package.json cli/Cargo.toml cli/Cargo.lock
  if [ "$SKIP_CHANGELOG" = false ] && [ -f CHANGELOG.md ]; then
    git add CHANGELOG.md
  fi
  git commit -m "chore: bump version to ${NEW_VERSION}"
  echo -e "${GREEN}Version bumped to ${NEW_VERSION} and committed.${NC}"
else
  echo -e "${GREEN}Version bumped to ${NEW_VERSION}.${NC}"
fi
