import matplotlib.pyplot as plt
import numpy as np

# Read the log file
with open('tx.log', 'r') as f:
    lines = f.readlines()

# Reference time and slot duration
ref_time = 1730563929.604366
slot_duration = 0.1  # 100ms

# Extract unique IPs and timestamps
senders = set()
data = []
for line in lines:
    parts = line.split()
    timestamp = float(parts[0])
    sender_ip = parts[2].split('.')[3]  # Get last octet of IP
    senders.add(sender_ip)
    data.append((timestamp, sender_ip))

# Sort senders for consistent y-axis ordering
senders = sorted(list(senders))

# Create figure
plt.figure(figsize=(15, 5))

# Plot pulses for each sender
for i, sender in enumerate(senders):
    timestamps = [t for t, ip in data if ip == sender]
    
    # Create step signal
    for t in timestamps:
        slot_start = ((t - ref_time) // slot_duration) * slot_duration + ref_time
        plt.plot([t, t + slot_duration/2], [i, i], 'b-', linewidth=2)

# Calculate x-axis limits and ticks
t_min = min(t for t, _ in data)
t_max = max(t for t, _ in data)
start_slot = ((t_min - ref_time) // slot_duration) * slot_duration
end_slot = ((t_max - ref_time) // slot_duration + 1) * slot_duration
tick_positions = np.arange(ref_time + start_slot, ref_time + end_slot + slot_duration, slot_duration)

# Customize plot
plt.yticks(range(len(senders)), [f'127.0.0.{ip}' for ip in senders])
plt.grid(True, axis='x', linestyle='--', alpha=0.7)
plt.xlabel('Time (s) from reference')
plt.title('UDP Transmission Slot Analysis')

# Set x-axis ticks at slot boundaries
plt.xticks(tick_positions, [f'{(t - ref_time):.3f}' for t in tick_positions])

plt.tight_layout()
plt.show()