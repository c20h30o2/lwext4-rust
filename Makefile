ARCE_DIR = ./lwext4_arce
CORE_DIR = ./lwext4_core
TEST_NAME ?= 
ARCE_INTEGRATION_TEST_TAG =--no-default-features --features use-rust 
test:
	cd ${ARCE_DIR} && cargo test ${ARCE_INTEGRATION_TEST_TAG} ${TEST_NAME} 
	
check-arce-status:
	cd $(ARCE_DIR) && cargo check ${ARCE_INTEGRATION_TEST_TAG}
check-core-status:
	cd ${CORE_DIR} && cargo check 