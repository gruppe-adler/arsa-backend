while getopts i:d:c:p: flag
do
    case "${flag}" in
        i) uuid="${OPTARG}";;
        d) directory="${OPTARG}";;
        c) config="${OPTARG}";;
        p) profile="${OPTARG}";;
    esac
done

cd "${directory}"
./ArmaReforgerServer -config "${config}" -profile "${profile}" -maxFPS 60 &
pid=$!
pidfile="${uuid}.pid"
echo "${pid}" > "${pidfile}"

# EXAMPLE
# =======
#./ars-start.sh \
# -i "0b6149a8-6e38-4914-8b9c-cc958c68acdb" \
# -d "/home/<username>/.local/share/Steam/steamapps/common/Arma Reforger Server/" \
# -c "/home/<username>/projects/arsa-backend/configs/0b6149a8-6e38-4914-8b9c-cc958c68acdb.json" \
# -p "/home/<username>/projects/arsa-backend/profiles/0b6149a8-6e38-4914-8b9c-cc958c68acdb/"