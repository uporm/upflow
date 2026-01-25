#!/usr/bin/env bash

set -e   # Exit immediately if any command fails

ROOT_DIR=$(pwd)

echo "📦 Rust workspace 单个项目发布脚本"
echo ""

# -----------------------------
# Discover workspace crates
# -----------------------------
echo "📚 获取 workspace crate 列表..."

# Build a JSON stream of {name, manifest_path} for workspace packages
PACKAGES_JSON=$(cargo metadata --no-deps --format-version=1 \
    | jq -c '.packages[] | select(.source == null) | {name: .name, path: .manifest_path}')

echo "🧩 发现以下 crates："
# Print the list for reference
echo "$PACKAGES_JSON" | jq -r '"- " + .name'
echo ""

# -----------------------------
# User input
# -----------------------------
read -p "请输入要发布的工程名称: " TARGET_NAME

if [ -z "$TARGET_NAME" ]; then
    echo "❌ 未输入工程名称，退出"
    exit 1
fi

# -----------------------------
# Locate target crate
# -----------------------------
# Use jq to pick the matching manifest_path
MANIFEST=$(echo "$PACKAGES_JSON" | jq -r --arg name "$TARGET_NAME" 'select(.name == $name) | .path')

if [ -z "$MANIFEST" ]; then
    echo "❌ 未找到名为 '$TARGET_NAME' 的工程"
    exit 1
fi

DIR=$(dirname "$MANIFEST")

echo ""
echo "=============================="
echo "📦 目标 crate: $TARGET_NAME"
echo "📁 路径 : $DIR"
echo "=============================="

cd "$DIR"

echo "🧪 执行 dry-run..."

# Capture stderr/stdout on failure
if ! OUTPUT=$(cargo publish --dry-run 2>&1); then
    echo "❌ dry-run 失败：$TARGET_NAME"
    echo "   👉 错误信息："
    echo "$OUTPUT"
    exit 1
fi

echo "✔ dry-run 成功：$TARGET_NAME"

# Publish directly without additional confirmation
echo "🚀 正在发布 $TARGET_NAME ..."

if ! OUTPUT=$(cargo publish 2>&1); then
    echo "❌ 发布失败：$TARGET_NAME"
    echo "   👉 错误信息："
    echo "$OUTPUT"
    exit 1
fi

echo "✅ 发布成功：$TARGET_NAME"

cd "$ROOT_DIR"
