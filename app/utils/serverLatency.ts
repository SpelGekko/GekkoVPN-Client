export async function measureLatency(serverIp: string): Promise<number> {
    const startTime = performance.now();
    
    try {
        // Try to connect to the OpenVPN port (1194)
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 2000); // 2 second timeout

        const response = await fetch(`http://${serverIp}:1194`, {
            mode: 'no-cors',
            cache: 'no-cache',
            signal: controller.signal
        });

        clearTimeout(timeoutId);
        return Math.round(performance.now() - startTime);
    } catch (error) {
        // Return high latency (999ms) on failure or timeout
        return 999;
    }
}