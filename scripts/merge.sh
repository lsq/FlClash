ul=$(git diff --name-only --diff-filter=U)

for f in $ul
do
    [ "$f" == "pubspec.yaml" ] && continue
    echo "$f--"
    git checkout --theirs -- "$f"
    git add "$f"
done
