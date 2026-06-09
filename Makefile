all: mqtt dashboard venv

mqtt:
	mosquitto -c mosquitto.conf -v

dashboard:
	cd fastapi && uvicorn app.main:app --reload

venv:
	source fastapi/.venv/bin/activate

.PHONY: mqtt dashboard venv
