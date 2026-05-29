all: mqtt dashboard

mqtt:
	mosquitto -c mosquitto.conf -v

dashboard:
	uvicorn fastapi/main:app --reload

.PHONY: mqtt dashboard
