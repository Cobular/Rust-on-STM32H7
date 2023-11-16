import matplotlib.pyplot as plt
from scipy.signal import find_peaks
import numpy as np
from pathlib import Path


# Read the series data from the file
parent_path = Path(__file__).parent.absolute()
with open(f"{parent_path}/fft_256_logs_nodc_half.txt", 'r') as file:
    x = np.array([float(line.strip()) for line in file.readlines() if line.strip() != ''])

x = np.concatenate((x, np.flip(x)))

# Find the peaks
peaks, _ = find_peaks(x, height=0.2e6)
print(len(peaks))


fig = plt.figure()
ax = fig.add_subplot(111)
# Plot the series data
plt.plot(x)
plt.plot(peaks, x[peaks], "x")


for peak_idx in peaks:
    print(peak_idx)
    ax.text(peak_idx, x[peak_idx]+500, f"{peak_idx,x[peak_idx]}", ha="center")


plt.xlabel('Index')
plt.ylabel('Value')
plt.title('Series Plot')
plt.show()