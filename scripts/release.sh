#!/bin/bash
set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <version>"
    exit 1
fi

VERSION=$1
CURRENT_BRANCH=$(git branch --show-current)

if [ "$CURRENT_BRANCH" != "main" ]; then
    echo "Error: Must be on main branch"
    exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
    echo "Error: Working directory is not clean"
    exit 1
fi

if ! command -v git-cliff &> /dev/null; then
    echo "Error: git-cliff is not installed"
    echo "Install it with: cargo install git-cliff"
    exit 1
fi

sed -i '' "s/^version = .*/version = \"$VERSION\"/" Cargo.toml

echo "Generating changelog..."
git-cliff --tag "v$VERSION" > CHANGELOG.md

echo "Preview of changes:"
echo "==================="
cat CHANGELOG.md
echo "==================="
read -p "Does this look good? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborting..."
    exit 1
fi

git add Cargo.toml CHANGELOG.md
git commit -m "chore(release): prepare for $VERSION"


git tag -a "v$VERSION" -m "Release version $VERSION"
git push origin main "v$VERSION"

echo "Release $VERSION created"