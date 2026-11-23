# Starts a background process, records its pid for the test, and waits for it.
sleep 30 &
echo "$!" > "$UPHILL_TEST_PID"
wait
