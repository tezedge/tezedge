import json
import matplotlib.pyplot as plt, mpld3
from pathlib import Path

history = json.load(open('./history.json', 'r'))
Path('./charts').mkdir(parents=True, exist_ok=True)

#fig, ax = plt.subplots()
#ax.grid(True, alpha=0.3)

fig = plt.figure(figsize=(20, 30))
fig.subplots_adjust(top=1, bottom=0.8, left=0.01, right=0.5)
ax = fig.add_subplot(1,1,1)

lns = []
labels = []

for name in history.keys():
    print(f'saving chart {name}')
    ln, = ax.plot(list(range(0, len(history[name]))), history[name], label=name)
    lns.append(ln)
    labels.append(name)

interactive_legend = mpld3.plugins.InteractiveLegendPlugin(lns, labels, start_visible=False)
mpld3.plugins.connect(fig, interactive_legend)
mpld3.save_html(fig, f'./charts/index.html')
