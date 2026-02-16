config.json: secrets.cue config.cue configSchema.cue
	cue export > config.json

.PHONY: clean

clean:
	rm -f config.json

